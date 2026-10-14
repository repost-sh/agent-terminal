use std::path::PathBuf;
use std::sync::{Arc, Weak};

use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::watch;

use super::stream::{hung_up, send};
use super::{Backend, tab_id};
use crate::native::pty;
use crate::native::recording;
use crate::native::store::SessionFolder;
use crate::native::terminal::Terminal;
use crate::output::call_line;
use crate::protocol::{Modes, Shell};

const SECRET_NAME_PARTS: [&str; 6] = ["TOKEN", "SECRET", "KEY", "PASSWORD", "PASSWD", "CREDENTIAL"];

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum ActivityUpdate {
    Entries { entries: Vec<Value> },
    Closed,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum OutputUpdate {
    Output { reset: bool, segments: Vec<Segment> },
    Closed,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum Segment {
    Text(String),
    Mark(String),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum InfoUpdate {
    Info(Box<Info>),
    Closed,
}

#[derive(Serialize)]
struct Info {
    program: String,
    args: String,
    pid: Option<u32>,
    cwd: String,
    term: Option<String>,
    cols: u16,
    rows: u16,
    shell_integration: Option<bool>,
    modes: Modes,
    recording: Option<PathBuf>,
    started: String,
    exited: bool,
    exit_code: Option<i32>,
    env: Vec<EnvVar>,
}

#[derive(Serialize)]
struct EnvVar {
    name: String,
    value: Option<String>,
}

struct Follow {
    backend: Arc<dyn Backend>,
    session: String,
    id: u32,
    lifecycle: watch::Receiver<u64>,
    folder: Weak<SessionFolder>,
    logged: Option<watch::Receiver<u64>>,
    watched: Option<(Arc<Terminal>, watch::Receiver<u64>)>,
}

impl Follow {
    fn new(backend: Arc<dyn Backend>, session: String, id: u32) -> Self {
        let folder = backend.folder(&session);
        Self {
            lifecycle: backend.changes(),
            logged: folder.as_ref().map(|folder| folder.logged()),
            folder: folder.as_ref().map_or_else(Weak::new, Arc::downgrade),
            backend,
            session,
            id,
            watched: None,
        }
    }

    fn tab(&self) -> String {
        format!("t{}", self.id)
    }

    fn terminal(&mut self) -> Option<(Arc<Terminal>, bool)> {
        self.lifecycle.borrow_and_update();
        if let Some(logged) = &mut self.logged {
            logged.borrow_and_update();
        }
        let terminal = self.backend.terminal(&self.session, self.id)?;
        let replaced = !self.watched.as_ref().is_some_and(|(watching, _)| Arc::ptr_eq(watching, &terminal));
        let (_, frames) = match &mut self.watched {
            Some(watched) if !replaced => watched,
            watched => watched.insert((Arc::clone(&terminal), terminal.frames())),
        };
        frames.borrow_and_update();
        Some((terminal, replaced))
    }

    async fn changed(&mut self, socket: &mut WebSocket) -> bool {
        let Self { lifecycle, logged, watched, .. } = self;
        let frames = changed_or_never(watched.as_mut().map(|(_, frames)| frames));
        let logged = changed_or_never(logged.as_mut());
        tokio::select! {
            changed = lifecycle.changed() => changed.is_ok(),
            changed = frames => changed.is_ok(),
            changed = logged => changed.is_ok(),
            received = socket.recv() => !hung_up(received),
        }
    }
}

async fn changed_or_never(receiver: Option<&mut watch::Receiver<u64>>) -> Result<(), watch::error::RecvError> {
    match receiver {
        Some(receiver) => receiver.changed().await,
        None => std::future::pending().await,
    }
}

pub async fn activity(
    ws: WebSocketUpgrade,
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab)): Path<(String, String)>,
) -> Response {
    match tab_id(&tab) {
        Some(id) => ws.on_upgrade(move |socket| follow_activity(socket, Follow::new(backend, session, id))),
        None => not_a_tab(&tab),
    }
}

async fn follow_activity(mut socket: WebSocket, mut follow: Follow) {
    let tab = follow.tab();
    let mut offset = 0;
    let mut sent_any = false;
    loop {
        let (Some(_), Some(folder)) = (follow.terminal(), follow.folder.upgrade()) else {
            send(&mut socket, &ActivityUpdate::Closed).await;
            return;
        };
        let read = match tokio::task::spawn_blocking(move || folder.activity_since(offset)).await {
            Ok(read) => read,
            Err(e) => Err(e.into()),
        };
        let entries = match read {
            Ok((entries, end)) => {
                offset = end;
                entries.into_iter().filter(|entry| entry["tab"] == tab).collect::<Vec<_>>()
            }
            Err(e) => {
                eprintln!("dashboard: reading the activity of {} {tab} failed: {e:#}", follow.session);
                return;
            }
        };
        if !sent_any || !entries.is_empty() {
            if !send(&mut socket, &ActivityUpdate::Entries { entries }).await {
                return;
            }
            sent_any = true;
        }
        if !follow.changed(&mut socket).await {
            return;
        }
    }
}

pub async fn output(
    ws: WebSocketUpgrade,
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab)): Path<(String, String)>,
) -> Response {
    match tab_id(&tab) {
        Some(id) => ws.on_upgrade(move |socket| follow_output(socket, Follow::new(backend, session, id))),
        None => not_a_tab(&tab),
    }
}

#[derive(Default)]
struct Streamed {
    offset: u64,
    undecoded: Vec<u8>,
    marks: Vec<(String, u64)>,
}

async fn follow_output(mut socket: WebSocket, mut follow: Follow) {
    let mut streamed: Option<Streamed> = None;
    loop {
        let Some((terminal, replaced)) = follow.terminal() else {
            send(&mut socket, &OutputUpdate::Closed).await;
            return;
        };
        if replaced {
            streamed = None;
        }
        if let Some(update) = output_update(&terminal, &mut streamed)
            && !send(&mut socket, &update).await
        {
            return;
        }
        if !follow.changed(&mut socket).await {
            return;
        }
    }
}

fn output_update(terminal: &Terminal, streamed: &mut Option<Streamed>) -> Option<OutputUpdate> {
    let from = streamed.as_ref().map_or(0, |streamed| streamed.offset);
    let mut raw = terminal.raw_output(from);
    let continues = streamed.take().filter(|streamed| {
        raw.start == streamed.offset
            && raw.marks.starts_with(&streamed.marks)
            && raw.marks[streamed.marks.len()..].iter().all(|(_, at)| *at >= streamed.offset)
    });
    let reset = continues.is_none();
    let Streamed { mut undecoded, marks: known, .. } = match continues {
        Some(continues) => continues,
        None => {
            raw = terminal.raw_output(0);
            Streamed::default()
        }
    };
    let mut segments = Vec::new();
    let mut taken = 0;
    for (name, at) in &raw.marks[known.len()..] {
        let Some(until) = at.checked_sub(raw.start) else { continue };
        let until = (until as usize).min(raw.bytes.len());
        let text = recording::decode(&mut undecoded, &raw.bytes[taken..until]);
        if !text.is_empty() {
            segments.push(Segment::Text(text));
        }
        segments.push(Segment::Mark(name.clone()));
        taken = until;
    }
    let text = recording::decode(&mut undecoded, &raw.bytes[taken..]);
    if !text.is_empty() {
        segments.push(Segment::Text(text));
    }
    *streamed = Some(Streamed { offset: raw.start + raw.bytes.len() as u64, undecoded, marks: raw.marks });
    (reset || !segments.is_empty()).then_some(OutputUpdate::Output { reset, segments })
}

pub async fn info(
    ws: WebSocketUpgrade,
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab)): Path<(String, String)>,
) -> Response {
    match tab_id(&tab) {
        Some(id) => ws.on_upgrade(move |socket| follow_info(socket, Follow::new(backend, session, id))),
        None => not_a_tab(&tab),
    }
}

async fn follow_info(mut socket: WebSocket, mut follow: Follow) {
    let mut sent = String::new();
    loop {
        let Some((terminal, _)) = follow.terminal() else {
            send(&mut socket, &InfoUpdate::Closed).await;
            return;
        };
        let info = match serde_json::to_string(&InfoUpdate::Info(Box::new(info_of(&terminal)))) {
            Ok(info) => info,
            Err(e) => {
                eprintln!("dashboard: encoding the info of {} {} failed: {e}", follow.session, follow.tab());
                return;
            }
        };
        if info != sent {
            if socket.send(axum::extract::ws::Message::text(info.clone())).await.is_err() {
                return;
            }
            sent = info;
        }
        if !follow.changed(&mut socket).await {
            return;
        }
    }
}

fn info_of(terminal: &Terminal) -> Info {
    let setup = terminal.setup();
    let env = pty::environment(&setup.command);
    let term = env.iter().find(|(name, _)| name == "TERM").map(|(_, value)| value.clone());
    Info {
        program: setup.command.program.clone(),
        args: call_line(&setup.command.args),
        pid: setup.pid,
        cwd: setup.cwd,
        term,
        cols: setup.size.cols,
        rows: setup.size.rows,
        shell_integration: match (terminal.shell(), Shell::of_program(&setup.command.program)) {
            (Some(_), _) => Some(true),
            (None, Some(_)) => Some(false),
            (None, None) => None,
        },
        modes: setup.modes,
        recording: terminal.recording().saving_to(),
        started: humantime::format_rfc3339_millis(setup.started).to_string(),
        exited: setup.exit.is_some(),
        exit_code: setup.exit.flatten(),
        env: masked(env),
    }
}

fn masked(mut env: Vec<(String, String)>) -> Vec<EnvVar> {
    env.sort();
    env.into_iter()
        .map(|(name, value)| {
            let upper = name.to_ascii_uppercase();
            let secret = SECRET_NAME_PARTS.iter().any(|part| upper.contains(part));
            EnvVar { value: (!secret).then_some(value), name }
        })
        .collect()
}

fn not_a_tab(tab: &str) -> Response {
    (StatusCode::NOT_FOUND, format!("{tab} is not a tab id; tabs are t1, t2, ...")).into_response()
}
