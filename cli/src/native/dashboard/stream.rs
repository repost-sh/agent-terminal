use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tokio::sync::watch;

use super::{Backend, tab_id};
use crate::native::terminal::{Frame, Terminal};
use crate::protocol::Size;

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

struct Watched {
    terminal: Arc<Terminal>,
    frames: watch::Receiver<u64>,
}

struct Shown {
    size: Size,
    rows: Vec<String>,
    frame: u64,
    exit: Option<Option<i32>>,
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

async fn follow_tab(mut socket: WebSocket, backend: Arc<dyn Backend>, session: String, id: u32) {
    let mut lifecycle = backend.changes();
    let mut watched: Option<Watched> = None;
    let mut shown: Option<Shown> = None;
    loop {
        lifecycle.borrow_and_update();
        let Some(terminal) = backend.terminal(&session, id) else {
            send(&mut socket, &Update::Closed).await;
            return;
        };
        if watched.as_ref().is_some_and(|watching| !Arc::ptr_eq(&watching.terminal, &terminal)) {
            watched = None;
            shown = None;
        }
        let watching = watched.get_or_insert_with(|| Watched { frames: terminal.frames(), terminal });
        watching.frames.borrow_and_update();
        let frame = watching.terminal.frame();
        if let Some(update) = update(&frame, shown.as_ref()) {
            if !send(&mut socket, &update).await {
                return;
            }
            shown = Some(Shown { size: frame.size, rows: frame.rows, frame: frame.number, exit: frame.exit });
        }
        tokio::select! {
            changed = lifecycle.changed() => if changed.is_err() { return },
            changed = watching.frames.changed() => if changed.is_err() { return },
            received = socket.recv() => if hung_up(received) { return },
        }
    }
}

fn update(frame: &Frame, shown: Option<&Shown>) -> Option<Update> {
    let repaint_all = shown.is_none_or(|shown| shown.size != frame.size);
    let same_screen =
        shown.is_some_and(|shown| !repaint_all && shown.frame == frame.number && shown.exit == frame.exit);
    if same_screen {
        return None;
    }
    let mut paint = String::new();
    for (index, row) in frame.rows.iter().enumerate() {
        let changed = repaint_all || shown.is_some_and(|shown| shown.rows.get(index) != Some(row));
        if changed {
            paint.push_str(&format!("\x1b[{};1H\x1b[0m\x1b[2K{row}", index + 1));
        }
    }
    let visibility = if frame.cursor_visible && frame.exit.is_none() { 'h' } else { 'l' };
    paint.push_str(&format!("\x1b[0m\x1b[{};{}H\x1b[?25{visibility}", frame.cursor.0, frame.cursor.1));
    Some(Update::Frame {
        frame: frame.number,
        cols: frame.size.cols,
        rows: frame.size.rows,
        paint,
        alt_screen: frame.alt_screen,
        mouse: frame.mouse,
        bracketed_paste: frame.bracketed_paste,
        exited: frame.exit.is_some(),
        exit_code: frame.exit.flatten(),
    })
}

async fn send(socket: &mut WebSocket, update: &Update) -> bool {
    match serde_json::to_string(update) {
        Ok(text) => socket.send(Message::text(text)).await.is_ok(),
        Err(e) => {
            eprintln!("dashboard: encoding a frame failed: {e}");
            false
        }
    }
}

pub fn hung_up(received: Option<Result<Message, axum::Error>>) -> bool {
    matches!(received, None | Some(Err(_)) | Some(Ok(Message::Close(_))))
}
