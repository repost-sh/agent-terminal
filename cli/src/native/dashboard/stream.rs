use std::sync::Arc;
use std::time::Duration;

use super::follow::{Event, Follow, Wake};
use super::{Backend, tab_id};
use crate::native::log;
use crate::native::terminal::{Glance, Picture, Terminal};
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

pub(super) async fn tab(
    ws: WebSocketUpgrade,
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab)): Path<(String, String)>,
) -> Response {
    match tab_id(&tab) {
        Some(id) => ws.on_upgrade(move |socket| follow_tab(socket, backend, session, id)),
        None => (StatusCode::NOT_FOUND, format!("{tab} is not a tab id; tabs are t1, t2, ...")).into_response(),
    }
}

#[derive(Default)]
struct Pace {
    in_flight: usize,
    last_sent: Option<Instant>,
    shown: Option<Shown>,
}

impl Pace {
    fn wake(&self, terminal: &Terminal) -> Option<Wake> {
        if self.in_flight >= WINDOW {
            return Some(Wake::OnMessage);
        }
        if self.shown.as_ref().is_some_and(|shown| shown.glance == terminal.glance()) {
            return Some(Wake::OnChange);
        }
        self.last_sent.map(|sent| sent + INTERVAL).filter(|due| *due > Instant::now()).map(Wake::At)
    }

    async fn show(&mut self, socket: &mut WebSocket, terminal: &Terminal) -> bool {
        let picture = terminal.picture();
        if !send(socket, &update(&picture, self.shown.as_ref())).await {
            return false;
        }
        self.shown = Some(Shown::of(picture));
        self.in_flight += 1;
        self.last_sent = Some(Instant::now());
        true
    }

    fn received(&mut self, text: &str) {
        if acknowledges(text) {
            self.in_flight = self.in_flight.saturating_sub(1);
        }
    }
}

async fn follow_tab(mut socket: WebSocket, backend: Arc<dyn Backend>, session: String, id: TabId) {
    let mut follow = Follow::tab(&backend, session, id, |terminal| Some(terminal.frames()));
    let mut pace = Pace::default();
    loop {
        let Some((terminal, replaced)) = follow.next() else {
            send(&mut socket, &Update::Closed).await;
            return;
        };
        if replaced {
            pace.shown = None;
        }
        let Some(wake) = pace.wake(&terminal) else {
            if !pace.show(&mut socket, &terminal).await {
                return;
            }
            continue;
        };
        match follow.event(&mut socket, wake).await {
            Event::Woke => {}
            Event::Received(text) => pace.received(&text),
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
    serde_json::from_str::<Received<'_>>(text).is_ok_and(|received| received.kind == "shown")
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
        exited: glance.status.exited(),
        exit_code: glance.status.code(),
    }
}

fn paint(picture: &Picture, shown: Option<&Shown>) -> String {
    picture.paint(shown.filter(|shown| shown.glance.size == picture.glance.size).map(|shown| shown.rows.as_slice()))
}

pub(super) async fn send(socket: &mut WebSocket, update: &impl Serialize) -> bool {
    match serde_json::to_string(update) {
        Ok(text) => socket.send(Message::text(text)).await.is_ok(),
        Err(e) => {
            log::note(format_args!("dashboard: encoding an update failed: {e}"));
            false
        }
    }
}

pub(super) async fn send_changed(socket: &mut WebSocket, sent: &mut String, update: String) -> bool {
    if update == *sent {
        return true;
    }
    if socket.send(Message::text(update.clone())).await.is_err() {
        return false;
    }
    *sent = update;
    true
}

pub(super) fn hung_up(received: Option<&Result<Message, axum::Error>>) -> bool {
    matches!(received, None | Some(Err(_) | Ok(Message::Close(_))))
}
