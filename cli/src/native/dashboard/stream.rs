use std::sync::Arc;

use super::follow::Follow;
use super::{Backend, tab_id};
use crate::native::terminal::Picture;
use crate::protocol::{Size, TabId};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

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
        if let Some(update) = update(&picture, shown.as_ref()) {
            if !send(&mut socket, &update).await {
                return;
            }
            shown = Some(Shown::of(picture));
        }
        if !follow.changed(&mut socket).await {
            return;
        }
    }
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
