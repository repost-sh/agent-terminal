use std::sync::Arc;

use axum::Json;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use super::{Backend, tab_id};
use crate::protocol::{ErrorBody, Request, Sessions, Shot, Target, Wait};

const RESTART_SETTLES_AFTER_MS: u64 = 300;
const RESTART_TIMEOUT_MS: u64 = 25_000;

pub async fn sessions(State(backend): State<Arc<dyn Backend>>) -> Json<Sessions> {
    Json(backend.sessions())
}

pub async fn sessions_stream(ws: WebSocketUpgrade, State(backend): State<Arc<dyn Backend>>) -> Response {
    ws.on_upgrade(move |socket| follow_sessions(socket, backend))
}

async fn follow_sessions(mut socket: WebSocket, backend: Arc<dyn Backend>) {
    let mut changes = backend.changes();
    let mut sent = String::new();
    loop {
        changes.borrow_and_update();
        let listed = match serde_json::to_string(&backend.sessions()) {
            Ok(listed) => listed,
            Err(e) => {
                eprintln!("dashboard: encoding the session list failed: {e}");
                return;
            }
        };
        if listed != sent {
            if socket.send(Message::text(listed.clone())).await.is_err() {
                return;
            }
            sent = listed;
        }
        tokio::select! {
            changed = changes.changed() => if changed.is_err() { return },
            received = socket.recv() => if super::stream::hung_up(received) { return },
        }
    }
}

pub async fn restart(
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab)): Path<(String, String)>,
) -> Response {
    let Some(id) = tab_id(&tab) else { return no_tab(&session, &tab) };
    let wait = Wait::Stable { quiet_ms: RESTART_SETTLES_AFTER_MS };
    call(backend, Request::Restart { target: Target::tab(&session, id), wait, timeout_ms: RESTART_TIMEOUT_MS }).await
}

pub async fn close(State(backend): State<Arc<dyn Backend>>, Path((session, tab)): Path<(String, String)>) -> Response {
    let Some(id) = tab_id(&tab) else { return no_tab(&session, &tab) };
    call(backend, Request::Close { target: Target::tab(&session, id), all: false }).await
}

pub async fn close_all(State(backend): State<Arc<dyn Backend>>) -> Response {
    call(backend, Request::Close { target: Target { session: String::new(), tab: None }, all: true }).await
}

pub async fn screenshot(
    State(backend): State<Arc<dyn Backend>>,
    Path((session, tab)): Path<(String, String)>,
) -> Response {
    let Some(id) = tab_id(&tab) else { return no_tab(&session, &tab) };
    call(backend, Request::Screenshot { target: Target::tab(&session, id), shot: Shot::default() }).await
}

pub async fn call(backend: Arc<dyn Backend>, request: Request) -> Response {
    let asked = serde_json::to_value(&request).map_or_else(|e| e.to_string(), |asked| asked["command"].to_string());
    match tokio::task::spawn_blocking(move || backend.call(request)).await {
        Ok(Ok(reply)) => Json(reply).into_response(),
        Ok(Err(error)) => {
            eprintln!("dashboard: {asked} from the dashboard failed: {}", error.message);
            refusal(error)
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("the daemon call failed: {e}")).into_response(),
    }
}

pub fn refused(message: String) -> Response {
    refusal(ErrorBody::failure(message))
}

fn refusal(error: ErrorBody) -> Response {
    (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({ "error": error }))).into_response()
}

fn no_tab(session: &str, tab: &str) -> Response {
    (StatusCode::NOT_FOUND, format!("{tab} is not a tab id in session {session}; tabs are t1, t2, ...")).into_response()
}
