use std::sync::Arc;

use super::Backend;
use super::follow::Follow;
use super::route::TabRoute;
use super::stream::send;
use crate::protocol::{CommandOutput, CommandRecord, Name, Shell};
use axum::Json;
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum Update {
    Commands { shell: bool, integration: bool, home: Option<String>, commands: Vec<CommandRecord> },
    Closed,
}

pub(super) async fn stream(ws: WebSocketUpgrade, route: TabRoute) -> Response {
    ws.on_upgrade(move |socket| follow_commands(socket, route))
}

async fn follow_commands(mut socket: WebSocket, route: TabRoute) {
    let mut follow = Follow::tab(&route).signal(|terminal| Some(terminal.history_changes()));
    let home = std::env::var("HOME").ok();
    loop {
        let Some((terminal, _)) = follow.next() else {
            send(&mut socket, &Update::Closed).await;
            return;
        };
        let integration = terminal.shell().is_some();
        let update = Update::Commands {
            shell: integration || Shell::of_program(terminal.program()).is_some(),
            integration,
            home: home.clone(),
            commands: terminal.commands().unwrap_or_default(),
        };
        if !send(&mut socket, &update).await || !follow.changed(&mut socket).await {
            return;
        }
    }
}

pub(super) async fn output(
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab, id)): Path<(String, String, u32)>,
) -> Response {
    let named: Option<(Name, Name)> = session.parse().ok().zip(tab.parse().ok());
    let open = named.and_then(|(session, tab)| {
        let terminal = backend.tab_id(&session, &tab).and_then(|id| backend.terminal(&session, id))?;
        Some((session, tab, terminal))
    });
    let Some((session, tab, terminal)) = open else {
        return (StatusCode::NOT_FOUND, format!("{session} {tab} is not open")).into_response();
    };
    match terminal.styled_command(id) {
        Some((record, output)) => Json(CommandOutput { session, tab, record, output, check: false }).into_response(),
        None => (StatusCode::NOT_FOUND, format!("no command {id} in {session} {tab}")).into_response(),
    }
}
