use std::sync::Arc;

use axum::Json;
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tokio::sync::watch;

use super::stream::{hung_up, send};
use super::{Backend, tab_id};
use crate::native::terminal::Terminal;
use crate::protocol::{CommandOutput, CommandRecord, Shell, TabId};

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum Update {
    Commands { shell: bool, integration: bool, home: Option<String>, commands: Vec<CommandRecord> },
    Closed,
}

struct Watched {
    terminal: Arc<Terminal>,
    history: watch::Receiver<u64>,
}

pub async fn stream(
    ws: WebSocketUpgrade,
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab)): Path<(String, String)>,
) -> Response {
    match tab_id(&tab) {
        Some(id) => ws.on_upgrade(move |socket| follow_commands(socket, backend, session, id)),
        None => not_a_tab(&tab),
    }
}

async fn follow_commands(mut socket: WebSocket, backend: Arc<dyn Backend>, session: String, id: TabId) {
    let mut lifecycle = backend.changes();
    let mut watched: Option<Watched> = None;
    let home = std::env::var("HOME").ok();
    loop {
        lifecycle.borrow_and_update();
        let Some(terminal) = backend.terminal(&session, id) else {
            send(&mut socket, &Update::Closed).await;
            return;
        };
        if watched.as_ref().is_some_and(|watching| !Arc::ptr_eq(&watching.terminal, &terminal)) {
            watched = None;
        }
        let watching = watched.get_or_insert_with(|| Watched { history: terminal.history_changes(), terminal });
        watching.history.borrow_and_update();
        let terminal = &watching.terminal;
        let integration = terminal.shell().is_some();
        let update = Update::Commands {
            shell: integration || Shell::of_program(&terminal.program).is_some(),
            integration,
            home: home.clone(),
            commands: terminal.commands().unwrap_or_default(),
        };
        if !send(&mut socket, &update).await {
            return;
        }
        tokio::select! {
            changed = lifecycle.changed() => if changed.is_err() { return },
            changed = watching.history.changed() => if changed.is_err() { return },
            received = socket.recv() => if hung_up(received) { return },
        }
    }
}

pub async fn output(
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab, id)): Path<(String, String, u32)>,
) -> Response {
    let Some(tab_id) = tab_id(&tab) else { return not_a_tab(&tab) };
    let Some(terminal) = backend.terminal(&session, tab_id) else {
        return (StatusCode::NOT_FOUND, format!("{session} {tab} is not open")).into_response();
    };
    match terminal.styled_command(id) {
        Some((record, output)) => Json(CommandOutput { session, tab, record, output, check: false }).into_response(),
        None => (StatusCode::NOT_FOUND, format!("no command {id} in {session} {tab}")).into_response(),
    }
}

fn not_a_tab(tab: &str) -> Response {
    (StatusCode::NOT_FOUND, format!("{tab} is not a tab id; tabs are t1, t2, ...")).into_response()
}
