use std::sync::{Arc, Weak};

use super::Backend;
use super::follow::Follow;
use super::route::TabRoute;
use super::stream::send;
use crate::native::log;
use crate::native::output_log::{Marked, Output, Slice};
use crate::native::pty;
use crate::native::store::{PendingCall, SessionFolder};
use crate::native::terminal::Terminal;
use crate::output::call_line;
use crate::protocol::{Modes, Shell};
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
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
enum InfoUpdate<'a> {
    Info(&'a Info),
    Closed,
}

#[derive(Serialize, PartialEq)]
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
    started: String,
    exited: bool,
    exit_code: Option<i32>,
    env: Vec<EnvVar>,
}

#[derive(Serialize, PartialEq)]
struct EnvVar {
    name: String,
    value: Option<String>,
}

pub(super) async fn activity(ws: WebSocketUpgrade, route: TabRoute) -> Response {
    ws.on_upgrade(move |socket| follow_activity(socket, route))
}

async fn follow_activity(mut socket: WebSocket, route: TabRoute) {
    let (session, tab) = (route.session.as_str(), route.id.to_string());
    let folder = route.backend.folder(session);
    let logged = folder.as_ref().map(|folder| folder.logged());
    let folder = folder.as_ref().map_or_else(Weak::new, Arc::downgrade);
    let mut follow = Follow::tab(&route).and(logged);
    let mut reported = Reported::default();
    loop {
        let (Some(_), Some(folder)) = (follow.next(), folder.upgrade()) else {
            send(&mut socket, &ActivityUpdate::Closed).await;
            return;
        };
        let Some(read) = read_activity(folder, reported.offset, session, &tab).await else {
            return;
        };
        if !(reported.report(&mut socket, read).await && follow.changed(&mut socket).await) {
            return;
        }
    }
}

#[derive(Default)]
struct Reported {
    offset: u64,
    sent: Option<Vec<PendingCall>>,
}

impl Reported {
    async fn report(&mut self, socket: &mut WebSocket, read: Read) -> bool {
        self.offset = read.end;
        if self.sent.as_ref() == Some(&read.pending) && read.entries.is_empty() {
            return true;
        }
        let delivered =
            send(socket, &ActivityUpdate::Entries { entries: read.entries, pending: read.pending.clone() }).await;
        self.sent = Some(read.pending);
        delivered
    }
}

struct Read {
    entries: Vec<Value>,
    end: u64,
    pending: Vec<PendingCall>,
}

async fn read_activity(folder: Arc<SessionFolder>, offset: u64, session: &str, tab: &str) -> Option<Read> {
    let read = match tokio::task::spawn_blocking(move || folder.activity_and_pending_since(offset)).await {
        Ok(read) => read,
        Err(e) => Err(e.into()),
    };
    let (entries, end, pending) = read
        .inspect_err(|e| log::note(format_args!("dashboard: reading the activity of {session} {tab} failed: {e:#}")))
        .ok()?;
    let entries = entries.into_iter().filter(|entry| entry.get("tab").is_some_and(|mine| *mine == tab)).collect();
    let pending = pending.into_iter().filter(|call| call.tab.as_deref() == Some(tab)).collect();
    Some(Read { entries, end, pending })
}

pub(super) async fn output(ws: WebSocketUpgrade, route: TabRoute) -> Response {
    ws.on_upgrade(move |socket| follow_output(socket, route))
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
            let until = usize::try_from(until).unwrap_or(usize::MAX).min(slice.bytes.len());
            let text = self.decode(slice.bytes.get(taken..until).unwrap_or_default());
            if !text.is_empty() {
                segments.push(Segment::Text(text));
            }
            segments.push(Segment::Mark(name.to_string()));
            taken = until;
        }
        let text = self.decode(slice.bytes.get(taken..).unwrap_or_default());
        if !text.is_empty() {
            segments.push(Segment::Text(text));
        }
        self.offset = slice.start + slice.bytes.len() as u64;
        segments
    }

    fn decode(&mut self, bytes: &[u8]) -> String {
        if self.undecoded.is_empty()
            && let Ok(whole) = std::str::from_utf8(bytes)
        {
            return whole.to_string();
        }
        let mut pending = std::mem::take(&mut self.undecoded);
        pending.extend_from_slice(bytes);
        let mut text = String::new();
        let mut rest = pending.as_slice();
        loop {
            match decode_step(rest, &mut text) {
                Decoded::Whole => return text,
                Decoded::Partial(after) => {
                    self.undecoded = after.to_vec();
                    return text;
                }
                Decoded::Replaced(after) => rest = after,
            }
        }
    }
}

enum Decoded<'a> {
    Whole,
    Partial(&'a [u8]),
    Replaced(&'a [u8]),
}

fn decode_step<'a>(rest: &'a [u8], text: &mut String) -> Decoded<'a> {
    let error = match std::str::from_utf8(rest) {
        Ok(valid) => {
            text.push_str(valid);
            return Decoded::Whole;
        }
        Err(error) => error,
    };
    let (valid, after) = rest.split_at_checked(error.valid_up_to()).unwrap_or((rest, &[]));
    text.push_str(&String::from_utf8_lossy(valid));
    match error.error_len() {
        Some(len) => {
            text.push(char::REPLACEMENT_CHARACTER);
            Decoded::Replaced(after.get(len..).unwrap_or_default())
        }
        None => Decoded::Partial(after),
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
            && output
                .marks
                .iter()
                .skip(self.marks.len())
                .all(|mark| mark.raw >= self.raw.offset && mark.styled >= self.styled.offset)
    }
}

async fn follow_output(mut socket: WebSocket, route: TabRoute) {
    let mut follow = Follow::tab(&route).signal(|terminal| Some(terminal.frames()));
    let mut streamed: Option<Streamed> = None;
    while let Some((terminal, replaced)) = follow.next() {
        let update = output_update(&terminal, &mut streamed, replaced);
        if !(send_any(&mut socket, update.as_ref()).await && follow.changed(&mut socket).await) {
            return;
        }
    }
    send(&mut socket, &OutputUpdate::Closed).await;
}

async fn send_any(socket: &mut WebSocket, update: Option<&OutputUpdate>) -> bool {
    match update {
        Some(update) => send(socket, update).await,
        None => true,
    }
}

fn output_update(terminal: &Terminal, streamed: &mut Option<Streamed>, replaced: bool) -> Option<OutputUpdate> {
    let (raw, styled) = streamed.as_ref().map_or((0, 0), |streamed| (streamed.raw.offset, streamed.styled.offset));
    let mut output = terminal.output_since(raw, styled);
    let continues = streamed.take().filter(|streamed| !replaced && streamed.continues(&output));
    let reset = continues.is_none();
    let mut streamed_now = if let Some(continues) = continues {
        continues
    } else {
        output = terminal.output_since(0, 0);
        Streamed::default()
    };
    let new = output.marks.get(streamed_now.marks.len()..).unwrap_or_default();
    let raw_marks: Vec<(u64, &str)> = new.iter().map(|mark| (mark.raw, mark.name.as_str())).collect();
    let styled_marks: Vec<(u64, &str)> = new.iter().map(|mark| (mark.styled, mark.name.as_str())).collect();
    let raw = streamed_now.raw.segments(&output.raw, &raw_marks);
    let styled = streamed_now.styled.segments(&output.styled, &styled_marks);
    streamed_now.marks = output.marks;
    *streamed = Some(streamed_now);
    (reset || !raw.is_empty() || !styled.is_empty()).then_some(OutputUpdate::Output { reset, styled, raw })
}

pub(super) async fn info(ws: WebSocketUpgrade, route: TabRoute) -> Response {
    ws.on_upgrade(move |socket| follow_info(socket, route))
}

async fn follow_info(mut socket: WebSocket, route: TabRoute) {
    let mut follow = Follow::tab(&route).signal(|terminal| Some(terminal.frames()));
    let mut sent: Option<Info> = None;
    while let Some((terminal, _)) = follow.next() {
        if !(send_new(&mut socket, &mut sent, info_of(&terminal)).await && follow.changed(&mut socket).await) {
            return;
        }
    }
    send(&mut socket, &InfoUpdate::Closed).await;
}

async fn send_new(socket: &mut WebSocket, sent: &mut Option<Info>, info: Info) -> bool {
    if sent.as_ref() == Some(&info) {
        return true;
    }
    let delivered = send(socket, &InfoUpdate::Info(&info)).await;
    *sent = Some(info);
    delivered
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
        started: humantime::format_rfc3339_millis(setup.started).to_string(),
        exited: setup.status.exited(),
        exit_code: setup.status.code(),
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

pub(super) async fn screenshot(
    State(backend): State<Arc<dyn Backend>>,
    Path((session, path)): Path<(String, String)>,
) -> Response {
    let not_listed = (StatusCode::NOT_FOUND, format!("{path} is not a PNG screenshot in session {session}"));
    let Some(folder) = backend.folder(&session) else {
        return not_listed.into_response();
    };
    let wanted = path.clone();
    let read = tokio::task::spawn_blocking(move || listed_png(&folder, &wanted)).await;
    match read {
        Ok(Ok(Some(png))) => ([(CONTENT_TYPE, "image/png"), (X_CONTENT_TYPE_OPTIONS, "nosniff")], png).into_response(),
        Ok(Ok(None)) => not_listed.into_response(),
        Ok(Err(e)) => (StatusCode::NOT_FOUND, format!("cannot read the screenshot {path}: {e:#}")).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("reading the screenshot {path} failed: {e}")).into_response()
        }
    }
}

fn listed_png(folder: &SessionFolder, wanted: &str) -> anyhow::Result<Option<Vec<u8>>> {
    let (entries, _) = folder.activity_since(0)?;
    let logged = entries
        .iter()
        .any(|entry| entry["screenshots"].as_array().is_some_and(|shots| shots.iter().any(|shot| shot == wanted)));
    let listed = logged || folder.screenshots()?.iter().any(|shot| shot.path.as_os_str() == wanted);
    if !listed || !wanted.to_ascii_lowercase().ends_with(".png") {
        return Ok(None);
    }
    Ok(Some(std::fs::read(wanted)?))
}

#[cfg(test)]
mod tests;
