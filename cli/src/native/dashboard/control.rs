use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::watch;

use super::follow::first;
use super::stream::send;
use super::{Backend, tab_id};
use crate::commands::without_global_options;
use crate::native::control::Hold;
use crate::native::log;
use crate::native::store::SessionFolder;
use crate::output::call_line;
use crate::protocol::{HumanInput, TabId};

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum ControlUpdate {
    Held,
    Refused { message: String },
    AgentRefused { command: String },
    Closed,
}

pub(super) async fn take(
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
                log::note(format_args!("dashboard: reading the activity of {tab} failed: {e:#}"));
                return None;
            }
            Err(e) => {
                log::note(format_args!("dashboard: reading the activity of {tab} failed: {e}"));
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
                log::note(format_args!("dashboard: reading the activity of {} failed: {e:#}", self.tab));
                return Vec::new();
            }
            Err(e) => {
                log::note(format_args!("dashboard: reading the activity of {} failed: {e}", self.tab));
                return Vec::new();
            }
        };
        entries
            .into_iter()
            .filter(|entry| self.refused_here(entry))
            .filter_map(|entry| entry.get("command").and_then(Value::as_str).and_then(shlex::split))
            .map(|typed| call_line(&without_global_options(&typed)))
            .collect()
    }

    fn refused_here(&self, entry: &Value) -> bool {
        entry.get("tab").is_some_and(|tab| *tab == self.tab)
            && entry.get("exit").is_some_and(|exit| *exit == crate::exit::HUMAN_CONTROL)
    }
}

async fn logged(refusals: Option<&mut Refusals>) -> Result<(), watch::error::RecvError> {
    match refusals {
        Some(refusals) => refusals.logged.changed().await,
        None => std::future::pending().await,
    }
}

struct Holding {
    backend: Arc<dyn Backend>,
    session: String,
    id: TabId,
    hold: Arc<Hold>,
    refusals: Option<Refusals>,
}

enum Next {
    Lifecycle(bool),
    Logged(bool),
    Received(Option<Result<Message, axum::Error>>),
}

async fn hold(mut socket: WebSocket, backend: Arc<dyn Backend>, session: String, id: TabId) {
    let Some(hold) = take_control(&mut socket, &backend, &session, id).await else {
        return;
    };
    let mut holding = Holding::start(backend, session, id, hold).await;
    if send(&mut socket, &ControlUpdate::Held).await {
        holding.run(&mut socket).await;
    }
}

async fn take_control(
    socket: &mut WebSocket,
    backend: &Arc<dyn Backend>,
    session: &str,
    id: TabId,
) -> Option<Arc<Hold>> {
    let taking = {
        let (backend, session) = (Arc::clone(backend), session.to_string());
        tokio::task::spawn_blocking(move || backend.take_control(&session, id)).await
    };
    match taking {
        Ok(Ok(hold)) => Some(Arc::new(hold)),
        Ok(Err(refused)) => {
            send(socket, &ControlUpdate::Refused { message: refused.message }).await;
            None
        }
        Err(e) => {
            log::note(format_args!("dashboard: taking control of {session} {id} failed: {e}"));
            None
        }
    }
}

impl Holding {
    async fn start(backend: Arc<dyn Backend>, session: String, id: TabId, hold: Arc<Hold>) -> Self {
        let refusals = match backend.folder(&session) {
            Some(folder) => Refusals::start(folder, id.to_string()).await,
            None => None,
        };
        Self { backend, session, id, hold, refusals }
    }

    async fn run(&mut self, socket: &mut WebSocket) {
        let mut lifecycle = self.backend.changes();
        while self.step(&mut lifecycle, socket).await {}
    }

    async fn step(&mut self, lifecycle: &mut watch::Receiver<u64>, socket: &mut WebSocket) -> bool {
        lifecycle.borrow_and_update();
        if self.backend.holds(&self.session, self.id, &self.hold) {
            return self.act(lifecycle, socket).await;
        }
        send(socket, &ControlUpdate::Closed).await;
        false
    }

    async fn act(&mut self, lifecycle: &mut watch::Receiver<u64>, socket: &mut WebSocket) -> bool {
        match self.next(lifecycle, socket).await {
            Next::Lifecycle(open) => open,
            Next::Logged(open) => open && self.tell_refusals(socket).await,
            Next::Received(received) => self.received(socket, received).await,
        }
    }

    async fn next(&mut self, lifecycle: &mut watch::Receiver<u64>, socket: &mut WebSocket) -> Next {
        let changed = first(async { Next::Lifecycle(lifecycle.changed().await.is_ok()) }, async {
            Next::Logged(logged(self.refusals.as_mut()).await.is_ok())
        });
        first(changed, async { Next::Received(socket.recv().await) }).await
    }

    async fn tell_refusals(&mut self, socket: &mut WebSocket) -> bool {
        let refused = match &mut self.refusals {
            Some(refusals) => refusals.new_ones().await,
            None => Vec::new(),
        };
        for command in refused {
            if !send(socket, &ControlUpdate::AgentRefused { command }).await {
                return false;
            }
        }
        true
    }

    async fn received(&self, socket: &mut WebSocket, received: Option<Result<Message, axum::Error>>) -> bool {
        let text = match received {
            Some(Ok(Message::Text(text))) => text,
            Some(Ok(Message::Binary(_) | Message::Ping(_) | Message::Pong(_))) => return true,
            None | Some(Err(_) | Ok(Message::Close(_))) => return false,
        };
        match self.type_keys(&text).await {
            Ok(()) => true,
            Err(message) => send(socket, &ControlUpdate::Refused { message }).await,
        }
    }

    async fn type_keys(&self, text: &str) -> Result<(), String> {
        let input = serde_json::from_str::<HumanInput>(text).map_err(|e| format!("{text} is not a key: {e}"))?;
        let (backend, session, hold, id) =
            (Arc::clone(&self.backend), self.session.clone(), Arc::clone(&self.hold), self.id);
        match tokio::task::spawn_blocking(move || backend.type_as_human(&session, id, &hold, &input)).await {
            Ok(typed) => typed.map_err(|refused| refused.message),
            Err(e) => Err(format!("sending the key failed: {e}")),
        }
    }
}
