use std::sync::Arc;
use std::time::Duration;

use super::follow::{Event, Follow, Wake};
use super::{Backend, tab_id};
use crate::native::terminal::{Glance, Picture};
use crate::protocol::TabId;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
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

struct Shown {
    glance: Glance,
    rows: Vec<String>,
}

impl Shown {
    fn of(picture: Picture) -> Self {
        Self { glance: picture.glance, rows: picture.rows }
    }
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
        let wake = if in_flight >= WINDOW {
            Wake::OnMessage
        } else if shown.as_ref().is_some_and(|shown| shown.glance == terminal.glance()) {
            Wake::OnChange
        } else {
            match last_sent.map(|sent| sent + INTERVAL).filter(|due| *due > Instant::now()) {
                Some(due) => Wake::At(due),
                None => {
                    let picture = terminal.picture();
                    if !send(&mut socket, &update(&picture, shown.as_ref())).await {
                        return;
                    }
                    shown = Some(Shown::of(picture));
                    in_flight += 1;
                    last_sent = Some(Instant::now());
                    continue;
                }
            }
        };
        match follow.event(&mut socket, wake).await {
            Event::Woke => {}
            Event::Received(text) => {
                if acknowledges(&text) {
                    in_flight = in_flight.saturating_sub(1);
                }
            }
            Event::HungUp => return,
        }
    }
}

#[derive(Deserialize)]
struct Received<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
}

fn acknowledges(text: &str) -> bool {
    serde_json::from_str::<Received>(text).is_ok_and(|received| received.kind == "shown")
}

fn update(picture: &Picture, shown: Option<&Shown>) -> Update {
    let glance = picture.glance;
    Update::Frame {
        frame: glance.frame,
        cols: glance.size.cols,
        rows: glance.size.rows,
        paint: paint(picture, shown),
        alt_screen: glance.alt_screen,
        mouse: glance.mouse,
        bracketed_paste: glance.bracketed_paste,
        exited: glance.exit.is_some(),
        exit_code: glance.exit.flatten(),
    }
}

fn paint(picture: &Picture, shown: Option<&Shown>) -> String {
    picture.paint(shown.filter(|shown| shown.glance.size == picture.glance.size).map(|shown| shown.rows.as_slice()))
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
