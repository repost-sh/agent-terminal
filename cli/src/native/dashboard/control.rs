use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tokio::sync::watch;

use super::stream::send;
use super::{Backend, tab_id};
use crate::native::store::SessionFolder;
use crate::protocol::{HumanInput, TabId};

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum ControlUpdate {
    Held,
    Refused { message: String },
    AgentRefused { command: String },
    Closed,
}

pub async fn take(
    ws: WebSocketUpgrade,
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab)): Path<(String, String)>,
) -> Response {
    match tab_id(&tab) {
        Some(id) => ws.on_upgrade(move |socket| hold(socket, backend, session, id)),
        None => (StatusCode::NOT_FOUND, format!("{tab} is not a tab id; tabs are t1, t2, ...")).into_response(),
    }
}

struct Refusals {
    folder: Arc<SessionFolder>,
    logged: watch::Receiver<u64>,
    offset: u64,
    tab: String,
}

impl Refusals {
    async fn start(folder: Arc<SessionFolder>, tab: String) -> Option<Self> {
        let mut logged = folder.logged();
        logged.borrow_and_update();
        let reading = Arc::clone(&folder);
        let offset = match tokio::task::spawn_blocking(move || reading.activity_since(0)).await {
            Ok(Ok((_, end))) => end,
            Ok(Err(e)) => {
                eprintln!("dashboard: reading the activity of {tab} failed: {e:#}");
                return None;
            }
            Err(e) => {
                eprintln!("dashboard: reading the activity of {tab} failed: {e}");
                return None;
            }
        };
        Some(Self { folder, logged, offset, tab })
    }

    async fn new_ones(&mut self) -> Vec<String> {
        self.logged.borrow_and_update();
        let (folder, offset) = (Arc::clone(&self.folder), self.offset);
        let entries = match tokio::task::spawn_blocking(move || folder.activity_since(offset)).await {
            Ok(Ok((entries, end))) => {
                self.offset = end;
                entries
            }
            Ok(Err(e)) => {
                eprintln!("dashboard: reading the activity of {} failed: {e:#}", self.tab);
                return Vec::new();
            }
            Err(e) => {
                eprintln!("dashboard: reading the activity of {} failed: {e}", self.tab);
                return Vec::new();
            }
        };
        entries
            .into_iter()
            .filter(|entry| entry["tab"] == self.tab && entry["exit"] == crate::exit::HUMAN_CONTROL)
            .filter_map(|entry| entry["command"].as_str().map(str::to_string))
            .collect()
    }
}

async fn logged(refusals: Option<&mut Refusals>) -> Result<(), watch::error::RecvError> {
    match refusals {
        Some(refusals) => refusals.logged.changed().await,
        None => std::future::pending().await,
    }
}

async fn hold(mut socket: WebSocket, backend: Arc<dyn Backend>, session: String, id: TabId) {
    let taking = {
        let (backend, session) = (Arc::clone(&backend), session.clone());
        tokio::task::spawn_blocking(move || backend.take_control(&session, id)).await
    };
    let hold = match taking {
        Ok(Ok(hold)) => Arc::new(hold),
        Ok(Err(refused)) => {
            send(&mut socket, &ControlUpdate::Refused { message: refused.message }).await;
            return;
        }
        Err(e) => {
            eprintln!("dashboard: taking control of {session} {id} failed: {e}");
            return;
        }
    };
    let tab = id.to_string();
    let mut refusals = match backend.folder(&session) {
        Some(folder) => Refusals::start(folder, tab.clone()).await,
        None => None,
    };
    let mut lifecycle = backend.changes();
    if !send(&mut socket, &ControlUpdate::Held).await {
        return;
    }
    loop {
        lifecycle.borrow_and_update();
        if !backend.holds(&session, id, &hold) {
            send(&mut socket, &ControlUpdate::Closed).await;
            return;
        }
        tokio::select! {
            changed = lifecycle.changed() => if changed.is_err() { return },
            changed = logged(refusals.as_mut()) => {
                if changed.is_err() {
                    return;
                }
                let refused = match &mut refusals {
                    Some(refusals) => refusals.new_ones().await,
                    None => Vec::new(),
                };
                for command in refused {
                    if !send(&mut socket, &ControlUpdate::AgentRefused { command }).await {
                        return;
                    }
                }
            }
            received = socket.recv() => {
                let text = match received {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(Message::Binary(_) | Message::Ping(_) | Message::Pong(_))) => continue,
                    None | Some(Err(_) | Ok(Message::Close(_))) => return,
                };
                let typed = match serde_json::from_str::<HumanInput>(&text) {
                    Ok(input) => {
                        let (backend, session, hold) = (Arc::clone(&backend), session.clone(), Arc::clone(&hold));
                        match tokio::task::spawn_blocking(move || backend.type_as_human(&session, id, &hold, &input)).await {
                            Ok(typed) => typed.map_err(|refused| refused.message),
                            Err(e) => Err(format!("sending the key failed: {e}")),
                        }
                    }
                    Err(e) => Err(format!("{text} is not a key: {e}")),
                };
                if let Err(message) = typed
                    && !send(&mut socket, &ControlUpdate::Refused { message }).await
                {
                    return;
                }
            }
        }
    }
}
