use std::sync::Arc;

use axum::Json;
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use serde_json::Value;
use tokio::sync::watch;

use super::Backend;
use super::stream::{LastSent, hung_up};
use crate::native::log;
use crate::protocol::request;
use crate::protocol::{ErrorBody, Request, Sessions, Shot, Target, Wait};

const RESTART_SETTLES_AFTER_MS: u64 = 300;
const RESTART_TIMEOUT_MS: u64 = 25_000;

pub(super) async fn sessions(State(backend): State<Arc<dyn Backend>>) -> Json<Sessions> {
    Json(backend.sessions())
}

pub(super) async fn sessions_stream(ws: WebSocketUpgrade, State(backend): State<Arc<dyn Backend>>) -> Response {
    ws.on_upgrade(move |socket| follow_sessions(socket, backend))
}

async fn follow_sessions(mut socket: WebSocket, backend: Arc<dyn Backend>) {
    let mut lifecycle = backend.changes();
    let mut sent = LastSent::default();
    loop {
        lifecycle.borrow_and_update();
        let Some(listed) = encoded_sessions(backend.as_ref()) else {
            return;
        };
        if !sent.send(&mut socket, listed).await {
            return;
        }
        if !still_open(&mut lifecycle, &mut socket).await {
            return;
        }
    }
}

async fn still_open(lifecycle: &mut watch::Receiver<u64>, socket: &mut WebSocket) -> bool {
    tokio::select! {
        changed = lifecycle.changed() => changed.is_ok(),
        received = socket.recv() => !hung_up(received.as_ref()),
    }
}

fn encoded_sessions(backend: &dyn Backend) -> Option<String> {
    serde_json::to_string(&backend.sessions())
        .inspect_err(|e| log::note(format_args!("dashboard: encoding the session list failed: {e}")))
        .ok()
}

pub(super) async fn restart(
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab)): Path<(String, String)>,
) -> Response {
    let Some(target) = addressed(&session, &tab) else { return no_tab(&session, &tab) };
    let wait = Wait::Stable { quiet_ms: RESTART_SETTLES_AFTER_MS };
    call(backend, Request::Restart(request::Restart { target, wait, timeout_ms: RESTART_TIMEOUT_MS })).await
}

pub(super) async fn close(
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab)): Path<(String, String)>,
) -> Response {
    let Some(target) = addressed(&session, &tab) else { return no_tab(&session, &tab) };
    call(backend, Request::Close(request::Close::Tab(target))).await
}

pub(super) async fn close_all(State(backend): State<Arc<dyn Backend>>) -> Response {
    call(backend, Request::Close(request::Close::EverySession)).await
}

pub(super) async fn screenshot(
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab)): Path<(String, String)>,
) -> Response {
    let Some(target) = addressed(&session, &tab) else { return no_tab(&session, &tab) };
    call(backend, Request::Screenshot(request::Screenshot { target, shot: Shot::default() })).await
}

fn addressed(session: &str, tab: &str) -> Option<Target> {
    Some(Target::tab(&session.parse().ok()?, &tab.parse().ok()?))
}

pub(super) async fn call(backend: Arc<dyn Backend>, request: Request) -> Response {
    let asked = serde_json::to_value(&request)
        .map_or_else(|e| e.to_string(), |asked| asked.get("command").unwrap_or(&Value::Null).to_string());
    match tokio::task::spawn_blocking(move || backend.call(request)).await {
        Ok(Ok(reply)) => Json(reply).into_response(),
        Ok(Err(error)) => {
            log::note(format_args!("dashboard: {asked} from the dashboard failed: {}", error.message));
            refusal(&error)
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("the daemon call failed: {e}")).into_response(),
    }
}

pub(super) fn refused(message: String) -> Response {
    refusal(&ErrorBody::failure(message))
}

fn refusal(error: &ErrorBody) -> Response {
    (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({ "error": error }))).into_response()
}

fn no_tab(session: &str, tab: &str) -> Response {
    (StatusCode::NOT_FOUND, format!("{session} {tab} is not a session and tab name")).into_response()
}
