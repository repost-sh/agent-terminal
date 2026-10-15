use std::sync::Arc;
use std::time::Duration;

use super::follow::{Event, Follow};
use super::{Backend, tab_id};
use crate::native::terminal::Picture;
use crate::protocol::{Size, TabId};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tokio::time::Instant;

const WINDOW: usize = 2;
const INTERVAL: Duration = Duration::from_millis(33);

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum Update {
    Frame {
        frame: u64,
        cols: u16,
        rows: u16,
        paint: String,
        alt_screen: bool,
        mouse: Option<&'static str>,
        bracketed_paste: bool,
        exited: bool,
        exit_code: Option<i32>,
    },
    Closed,
}

type Modes = (bool, Option<&'static str>, bool);

pub struct Shown {
    size: Size,
    rows: Vec<String>,
    frame: u64,
    modes: Modes,
    exit: Option<Option<i32>>,
}

impl Shown {
    pub fn of(picture: Picture) -> Self {
        Self {
            modes: modes(&picture),
            size: picture.size,
            rows: picture.rows,
            frame: picture.frame,
            exit: picture.exit,
        }
    }

    pub fn size(&self) -> Size {
        self.size
    }
}

fn modes(picture: &Picture) -> Modes {
    (picture.alt_screen, picture.mouse, picture.bracketed_paste)
}

pub async fn tab(
    ws: WebSocketUpgrade,
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab)): Path<(String, String)>,
) -> Response {
    match tab_id(&tab) {
        Some(id) => ws.on_upgrade(move |socket| follow_tab(socket, backend, session, id)),
        None => (StatusCode::NOT_FOUND, format!("{tab} is not a tab id; tabs are t1, t2, ...")).into_response(),
    }
}

async fn follow_tab(mut socket: WebSocket, backend: Arc<dyn Backend>, session: String, id: TabId) {
    let mut follow = Follow::tab(&backend, session, id, |terminal| Some(terminal.frames()));
    let mut in_flight = 0usize;
    let mut last_sent: Option<Instant> = None;
    let mut shown: Option<Shown> = None;
    loop {
        let Some((terminal, replaced)) = follow.next() else {
            send(&mut socket, &Update::Closed).await;
            return;
        };
        if replaced {
            shown = None;
        }
        let picture = terminal.picture();
        let pending = update(&picture, shown.as_ref());
        let ready_at = last_sent.map(|sent| sent + INTERVAL);
        let may_send = in_flight < WINDOW && ready_at.is_none_or(|at| Instant::now() >= at);
        if let (Some(update), true) = (&pending, may_send) {
            if !send(&mut socket, update).await {
                return;
            }
            shown = Some(Shown::of(picture));
            in_flight += 1;
            last_sent = Some(Instant::now());
            continue;
        }
        let deadline = if pending.is_some() && in_flight < WINDOW { ready_at } else { None };
        match follow.event(&mut socket, deadline).await {
            Event::Changed | Event::Ignored | Event::Deadline => {}
            Event::Received(text) => {
                if acknowledges(&text) {
                    in_flight = in_flight.saturating_sub(1);
                }
            }
            Event::HungUp => return,
        }
    }
}

fn acknowledges(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text).is_ok_and(|message| message["type"] == "shown")
}

fn update(picture: &Picture, shown: Option<&Shown>) -> Option<Update> {
    let unchanged = shown.is_some_and(|shown| {
        shown.size == picture.size
            && shown.frame == picture.frame
            && shown.exit == picture.exit
            && shown.modes == modes(picture)
    });
    if unchanged {
        return None;
    }
    Some(Update::Frame {
        frame: picture.frame,
        cols: picture.size.cols,
        rows: picture.size.rows,
        paint: paint(picture, shown),
        alt_screen: picture.alt_screen,
        mouse: picture.mouse,
        bracketed_paste: picture.bracketed_paste,
        exited: picture.exit.is_some(),
        exit_code: picture.exit.flatten(),
    })
}

pub fn paint(picture: &Picture, shown: Option<&Shown>) -> String {
    picture.paint(shown.filter(|shown| shown.size == picture.size).map(|shown| shown.rows.as_slice()))
}

pub async fn send(socket: &mut WebSocket, update: &impl Serialize) -> bool {
    match serde_json::to_string(update) {
        Ok(text) => socket.send(Message::text(text)).await.is_ok(),
        Err(e) => {
            eprintln!("dashboard: encoding an update failed: {e}");
            false
        }
    }
}

pub fn hung_up(received: Option<Result<Message, axum::Error>>) -> bool {
    matches!(received, None | Some(Err(_)) | Some(Ok(Message::Close(_))))
}
