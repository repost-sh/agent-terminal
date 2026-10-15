use std::sync::Arc;

use anyhow::Result;
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tokio::sync::watch;

use super::stream::{Shown, hung_up, paint, send};
use super::{Backend, tab_id};
use crate::native::commands::epoch_ms;
use crate::native::replay::Replay;
use crate::native::terminal::{Run, Terminal};

const KEYFRAME_EVERY: u64 = 32;
const FRAMES_PER_MESSAGE: usize = 256;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum Update {
    Frames { reset: bool, started_ms: u64, frames: Vec<Frame> },
    Failed { message: String },
    Closed,
}

#[derive(Serialize)]
struct Frame {
    frame: u64,
    time_ms: u64,
    keyframe: bool,
    cols: u16,
    rows: u16,
    paint: String,
    alt_screen: bool,
    mouse: Option<&'static str>,
    bracketed_paste: bool,
}

struct Cursor {
    replay: Replay,
    shown: Option<Shown>,
}

impl Cursor {
    fn start(run: &Run) -> Result<Self> {
        Ok(Self { replay: Replay::start(run)?, shown: None })
    }

    fn frames(&mut self, ends: &[u64]) -> Result<Vec<Frame>> {
        ends.iter().map(|end| self.frame(*end)).collect()
    }

    fn frame(&mut self, end: u64) -> Result<Frame> {
        let drawn = self.replay.next_frame(end)?;
        let picture = drawn.picture;
        let resized = self.shown.as_ref().is_none_or(|shown| shown.size() != picture.size);
        let keyframe = resized || (picture.frame - 1) % KEYFRAME_EVERY == 0;
        let frame = Frame {
            frame: picture.frame,
            time_ms: drawn.time_ms,
            keyframe,
            cols: picture.size.cols,
            rows: picture.size.rows,
            paint: paint(&picture, if keyframe { None } else { self.shown.as_ref() }),
            alt_screen: picture.alt_screen,
            mouse: picture.mouse,
            bracketed_paste: picture.bracketed_paste,
        };
        self.shown = Some(Shown::of(picture));
        Ok(frame)
    }
}

struct Following {
    terminal: Arc<Terminal>,
    frames: watch::Receiver<u64>,
    run: Run,
    cursor: Option<Cursor>,
    sent: usize,
    reset: bool,
    failed: bool,
}

struct CaughtUp {
    update: Option<Update>,
    more: bool,
}

impl Following {
    fn new(terminal: Arc<Terminal>) -> Self {
        Self {
            frames: terminal.frames(),
            run: terminal.run(),
            terminal,
            cursor: None,
            sent: 0,
            reset: true,
            failed: false,
        }
    }

    async fn catch_up(&mut self, name: &str) -> CaughtUp {
        let mut ends = self.terminal.frame_ends(self.sent);
        if self.failed || (ends.is_empty() && !self.reset) {
            return CaughtUp { update: None, more: false };
        }
        let more = ends.len() > FRAMES_PER_MESSAGE;
        ends.truncate(FRAMES_PER_MESSAGE);
        let run = self.run.clone();
        let cursor = self.cursor.take();
        let replaying = tokio::task::spawn_blocking(move || -> Result<(Cursor, Vec<Frame>)> {
            let mut cursor = match cursor {
                Some(cursor) => cursor,
                None => Cursor::start(&run)?,
            };
            let frames = cursor.frames(&ends)?;
            Ok((cursor, frames))
        });
        let replayed = match replaying.await {
            Ok(replayed) => replayed,
            Err(e) => Err(anyhow::Error::new(e).context("the replay stopped")),
        };
        match replayed {
            Ok((cursor, frames)) => {
                self.cursor = Some(cursor);
                self.sent += frames.len();
                let reset = std::mem::take(&mut self.reset);
                let started_ms = epoch_ms(self.run.started);
                CaughtUp { update: Some(Update::Frames { reset, started_ms, frames }), more }
            }
            Err(e) => {
                self.failed = true;
                let message = format!("the timeline of {name} could not be read from its recording: {e:#}");
                eprintln!("dashboard: {message}");
                CaughtUp { update: Some(Update::Failed { message }), more: false }
            }
        }
    }
}

pub async fn tab(
    ws: WebSocketUpgrade,
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab)): Path<(String, String)>,
) -> Response {
    match tab_id(&tab) {
        Some(id) => ws.on_upgrade(move |socket| follow_timeline(socket, backend, session, id)),
        None => (StatusCode::NOT_FOUND, format!("{tab} is not a tab id; tabs are t1, t2, ...")).into_response(),
    }
}

async fn follow_timeline(mut socket: WebSocket, backend: Arc<dyn Backend>, session: String, id: u32) {
    let name = format!("{session} t{id}");
    let mut lifecycle = backend.changes();
    let mut following: Option<Following> = None;
    loop {
        lifecycle.borrow_and_update();
        let Some(terminal) = backend.terminal(&session, id) else {
            send(&mut socket, &Update::Closed).await;
            return;
        };
        if following.as_ref().is_some_and(|follow| !Arc::ptr_eq(&follow.terminal, &terminal)) {
            following = None;
        }
        let follow = following.get_or_insert_with(|| Following::new(terminal));
        follow.frames.borrow_and_update();
        let caught_up = follow.catch_up(&name).await;
        if let Some(update) = caught_up.update
            && !send(&mut socket, &update).await
        {
            return;
        }
        if caught_up.more {
            continue;
        }
        tokio::select! {
            changed = lifecycle.changed() => if changed.is_err() { return },
            changed = follow.frames.changed() => if changed.is_err() { return },
            received = socket.recv() => if hung_up(received) { return },
        }
    }
}
