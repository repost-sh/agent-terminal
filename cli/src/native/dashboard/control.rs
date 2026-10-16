use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::watch;

use super::Backend;
use super::follow::first;
use super::route::TabRoute;
use super::stream::send;
use crate::global_options::without_global_options;
use crate::native::control::TabHold;
use crate::native::log;
use crate::native::store::{ActivityEntry, SessionFolder, TabId};
use crate::output::call_line;
use crate::protocol::HumanInput;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum ControlUpdate {
    Held,
    Refused { message: String },
    AgentRefused { command: String },
    Closed,
}

pub(super) async fn take(ws: WebSocketUpgrade, route: TabRoute) -> Response {
    ws.on_upgrade(move |socket| hold(socket, route))
}

struct Refusals {
    folder: Arc<SessionFolder>,
    logged: watch::Receiver<u64>,
    offset: u64,
    tab: TabId,
}

impl Refusals {
    async fn start(folder: Arc<SessionFolder>, tab: TabId) -> Option<Self> {
        let mut logged = folder.logged();
        logged.borrow_and_update();
        let reading = Arc::clone(&folder);
        let offset = match tokio::task::spawn_blocking(move || reading.activity_since(0)).await {
            Ok(Ok((_, end))) => end,
            Ok(Err(e)) => {
                log::note(format_args!("dashboard: reading the activity of a held tab failed: {e:#}"));
                return None;
            }
            Err(e) => {
                log::note(format_args!("dashboard: reading the activity of a held tab failed: {e}"));
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
                log::note(format_args!("dashboard: reading the activity of a held tab failed: {e:#}"));
                return Vec::new();
            }
            Err(e) => {
                log::note(format_args!("dashboard: reading the activity of a held tab failed: {e}"));
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
        ActivityEntry::logged_for(entry, self.tab)
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
    hold: Arc<TabHold>,
    refusals: Option<Refusals>,
}

enum Next {
    Lifecycle(bool),
    Logged(bool),
    Received(Option<Result<Message, axum::Error>>),
}

async fn hold(mut socket: WebSocket, route: TabRoute) {
    let Some(hold) = take_control(&mut socket, &route).await else {
        return;
    };
    let mut holding = Holding::start(route.backend, hold).await;
    if send(&mut socket, &ControlUpdate::Held).await {
        holding.run(&mut socket).await;
    }
}

async fn take_control(socket: &mut WebSocket, route: &TabRoute) -> Option<Arc<TabHold>> {
    let TabRoute { backend, session, name, .. } = route;
    let taking = {
        let (backend, session, name) = (Arc::clone(backend), session.clone(), name.clone());
        tokio::task::spawn_blocking(move || backend.take_control(&session, &name)).await
    };
    match taking {
        Ok(Ok(hold)) => Some(Arc::new(hold)),
        Ok(Err(refused)) => {
            send(socket, &ControlUpdate::Refused { message: refused.message }).await;
            None
        }
        Err(e) => {
            log::note(format_args!("dashboard: taking control of {session} {name} failed: {e}"));
            None
        }
    }
}

impl Holding {
    async fn start(backend: Arc<dyn Backend>, hold: Arc<TabHold>) -> Self {
        let refusals = match backend.folder(&hold.session) {
            Some(folder) => Refusals::start(folder, hold.tab).await,
            None => None,
        };
        Self { backend, hold, refusals }
    }

    async fn run(&mut self, socket: &mut WebSocket) {
        let mut lifecycle = self.backend.changes();
        while self.step(&mut lifecycle, socket).await {}
    }

    async fn step(&mut self, lifecycle: &mut watch::Receiver<u64>, socket: &mut WebSocket) -> bool {
        lifecycle.borrow_and_update();
        if self.backend.holds(&self.hold) {
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
        let (backend, hold) = (Arc::clone(&self.backend), Arc::clone(&self.hold));
        match tokio::task::spawn_blocking(move || backend.type_as_human(&hold, &input)).await {
            Ok(typed) => typed.map_err(|refused| refused.message),
            Err(e) => Err(format!("sending the key failed: {e}")),
        }
    }
}
