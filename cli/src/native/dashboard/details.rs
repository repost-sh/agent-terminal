use std::path::PathBuf;
use std::sync::{Arc, Weak};

use super::follow::Follow;
use super::stream::send;
use super::{Backend, tab_id};
use crate::native::output_log::{Marked, Output, Slice};
use crate::native::pty;
use crate::native::recording;
use crate::native::store::PendingCall;
use crate::native::terminal::Terminal;
use crate::output::call_line;
use crate::protocol::{Modes, Shell, TabId};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header::{CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::Value;

const SECRET_NAME_PARTS: [&str; 6] = ["TOKEN", "SECRET", "KEY", "PASSWORD", "PASSWD", "CREDENTIAL"];

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum ActivityUpdate {
    Entries { entries: Vec<Value>, pending: Vec<PendingCall> },
    Closed,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum OutputUpdate {
    Output { reset: bool, styled: Vec<Segment>, raw: Vec<Segment> },
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

pub async fn activity(
    ws: WebSocketUpgrade,
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab)): Path<(String, String)>,
) -> Response {
    match tab_id(&tab) {
        Some(id) => ws.on_upgrade(move |socket| follow_activity(socket, backend, session, id)),
        None => not_a_tab(&tab),
    }
}

async fn follow_activity(mut socket: WebSocket, backend: Arc<dyn Backend>, session: String, id: TabId) {
    let tab = id.to_string();
    let folder = backend.folder(&session);
    let logged = folder.as_ref().map(|folder| folder.logged());
    let folder = folder.as_ref().map_or_else(Weak::new, Arc::downgrade);
    let mut follow = Follow::tab(&backend, session.clone(), id, |_| None).and(logged);
    let mut offset = 0;
    let mut sent: Option<Vec<PendingCall>> = None;
    loop {
        let (Some(_), Some(folder)) = (follow.next(), folder.upgrade()) else {
            send(&mut socket, &ActivityUpdate::Closed).await;
            return;
        };
        let read = match tokio::task::spawn_blocking(move || folder.activity_and_pending_since(offset)).await {
            Ok(read) => read,
            Err(e) => Err(e.into()),
        };
        let (entries, pending) = match read {
            Ok((entries, end, pending)) => {
                offset = end;
                let mine = |entry: &serde_json::Value| entry["tab"] == tab;
                let pending: Vec<PendingCall> =
                    pending.into_iter().filter(|call| call.tab.as_deref() == Some(tab.as_str())).collect();
                (entries.into_iter().filter(mine).collect::<Vec<_>>(), pending)
            }
            Err(e) => {
                eprintln!("dashboard: reading the activity of {session} {tab} failed: {e:#}");
                return;
            }
        };
        if sent.as_ref() != Some(&pending) || !entries.is_empty() {
            if !send(&mut socket, &ActivityUpdate::Entries { entries, pending: pending.clone() }).await {
                return;
            }
            sent = Some(pending);
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
        Some(id) => ws.on_upgrade(move |socket| follow_output(socket, backend, session, id)),
        None => not_a_tab(&tab),
    }
}

#[derive(Default)]
struct Stream {
    offset: u64,
    undecoded: Vec<u8>,
}

impl Stream {
    fn segments(&mut self, slice: &Slice, marks: &[(u64, &str)]) -> Vec<Segment> {
        let mut segments = Vec::new();
        let mut taken = 0;
        for &(at, name) in marks {
            let Some(until) = at.checked_sub(slice.start) else { continue };
            let until = (until as usize).min(slice.bytes.len());
            let text = recording::decode(&mut self.undecoded, &slice.bytes[taken..until]);
            if !text.is_empty() {
                segments.push(Segment::Text(text));
            }
            segments.push(Segment::Mark(name.to_string()));
            taken = until;
        }
        let text = recording::decode(&mut self.undecoded, &slice.bytes[taken..]);
        if !text.is_empty() {
            segments.push(Segment::Text(text));
        }
        self.offset = slice.start + slice.bytes.len() as u64;
        segments
    }
}

#[derive(Default)]
struct Streamed {
    raw: Stream,
    styled: Stream,
    marks: Vec<Marked>,
}

impl Streamed {
    fn continues(&self, output: &Output) -> bool {
        output.raw.start == self.raw.offset
            && output.styled.start == self.styled.offset
            && output.marks.starts_with(&self.marks)
            && output.marks[self.marks.len()..]
                .iter()
                .all(|mark| mark.raw >= self.raw.offset && mark.styled >= self.styled.offset)
    }
}

async fn follow_output(mut socket: WebSocket, backend: Arc<dyn Backend>, session: String, id: TabId) {
    let mut follow = Follow::tab(&backend, session, id, |terminal| Some(terminal.frames()));
    let mut streamed: Option<Streamed> = None;
    loop {
        let Some((terminal, replaced)) = follow.next() else {
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
    let (raw, styled) = streamed.as_ref().map_or((0, 0), |streamed| (streamed.raw.offset, streamed.styled.offset));
    let mut output = terminal.output_since(raw, styled);
    let continues = streamed.take().filter(|streamed| streamed.continues(&output));
    let reset = continues.is_none();
    let mut streamed_now = match continues {
        Some(continues) => continues,
        None => {
            output = terminal.output_since(0, 0);
            Streamed::default()
        }
    };
    let new = &output.marks[streamed_now.marks.len()..];
    let raw_marks: Vec<(u64, &str)> = new.iter().map(|mark| (mark.raw, mark.name.as_str())).collect();
    let styled_marks: Vec<(u64, &str)> = new.iter().map(|mark| (mark.styled, mark.name.as_str())).collect();
    let raw = streamed_now.raw.segments(&output.raw, &raw_marks);
    let styled = streamed_now.styled.segments(&output.styled, &styled_marks);
    streamed_now.marks = output.marks;
    *streamed = Some(streamed_now);
    (reset || !raw.is_empty() || !styled.is_empty()).then_some(OutputUpdate::Output { reset, styled, raw })
}

pub async fn info(
    ws: WebSocketUpgrade,
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab)): Path<(String, String)>,
) -> Response {
    match tab_id(&tab) {
        Some(id) => ws.on_upgrade(move |socket| follow_info(socket, backend, session, id)),
        None => not_a_tab(&tab),
    }
}

async fn follow_info(mut socket: WebSocket, backend: Arc<dyn Backend>, session: String, id: TabId) {
    let mut follow = Follow::tab(&backend, session.clone(), id, |terminal| Some(terminal.frames()));
    let mut sent = String::new();
    loop {
        let Some((terminal, _)) = follow.next() else {
            send(&mut socket, &InfoUpdate::Closed).await;
            return;
        };
        let info = match serde_json::to_string(&InfoUpdate::Info(Box::new(info_of(&terminal)))) {
            Ok(info) => info,
            Err(e) => {
                eprintln!("dashboard: encoding the info of {session} {id} failed: {e}");
                return;
            }
        };
        if info != sent {
            if socket.send(Message::text(info.clone())).await.is_err() {
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

pub async fn screenshot(
    State(backend): State<Arc<dyn Backend>>,
    Path((session, path)): Path<(String, String)>,
) -> Response {
    let not_listed = (StatusCode::NOT_FOUND, format!("{path} is not a PNG screenshot in session {session}"));
    let Some(folder) = backend.folder(&session) else {
        return not_listed.into_response();
    };
    let wanted = path.clone();
    let read = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<Vec<u8>>> {
        let (entries, _) = folder.activity_since(0)?;
        let logged = entries.iter().any(|entry| {
            entry["screenshots"].as_array().is_some_and(|shots| shots.iter().any(|shot| shot == wanted.as_str()))
        });
        let listed = logged || folder.screenshots()?.iter().any(|shot| shot.path.as_os_str() == wanted.as_str());
        if !listed || !wanted.to_ascii_lowercase().ends_with(".png") {
            return Ok(None);
        }
        Ok(Some(std::fs::read(&wanted)?))
    })
    .await;
    match read {
        Ok(Ok(Some(png))) => ([(CONTENT_TYPE, "image/png"), (X_CONTENT_TYPE_OPTIONS, "nosniff")], png).into_response(),
        Ok(Ok(None)) => not_listed.into_response(),
        Ok(Err(e)) => (StatusCode::NOT_FOUND, format!("cannot read the screenshot {path}: {e:#}")).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("reading the screenshot {path} failed: {e}")).into_response()
        }
    }
}

fn not_a_tab(tab: &str) -> Response {
    (StatusCode::NOT_FOUND, format!("{tab} is not a tab id; tabs are t1, t2, ...")).into_response()
}
