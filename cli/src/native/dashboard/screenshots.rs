use std::sync::Arc;

use anyhow::Context;
use axum::Json;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::Backend;
use super::stream::{hung_up, send};
use crate::native::render::{self, CellBox};
use crate::native::store::{ScreenshotEntry, SessionFolder};

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum Update {
    Screenshots { screenshots: Vec<ScreenshotEntry> },
    Closed,
}

struct Watched {
    folder: Arc<SessionFolder>,
    listed: watch::Receiver<u64>,
}

pub async fn stream(
    ws: WebSocketUpgrade,
    State(backend): State<Arc<dyn Backend>>,
    Path(session): Path<String>,
) -> Response {
    ws.on_upgrade(move |socket| follow_screenshots(socket, backend, session))
}

async fn follow_screenshots(mut socket: WebSocket, backend: Arc<dyn Backend>, session: String) {
    let mut lifecycle = backend.changes();
    let mut watched: Option<Watched> = None;
    let mut sent = String::new();
    loop {
        lifecycle.borrow_and_update();
        let Some(folder) = backend.folder(&session) else {
            send(&mut socket, &Update::Closed).await;
            return;
        };
        if watched.as_ref().is_some_and(|watching| !Arc::ptr_eq(&watching.folder, &folder)) {
            watched = None;
        }
        let watching = watched.get_or_insert_with(|| Watched { listed: folder.screenshots_listed(), folder });
        watching.listed.borrow_and_update();
        let folder = Arc::clone(&watching.folder);
        let screenshots = match tokio::task::spawn_blocking(move || folder.screenshots()).await {
            Ok(Ok(screenshots)) => screenshots,
            Ok(Err(e)) => {
                eprintln!("dashboard: reading the screenshots of {session} failed: {e:#}");
                return;
            }
            Err(e) => {
                eprintln!("dashboard: reading the screenshots of {session} failed: {e}");
                return;
            }
        };
        let update = match serde_json::to_string(&Update::Screenshots { screenshots }) {
            Ok(update) => update,
            Err(e) => {
                eprintln!("dashboard: encoding the screenshots of {session} failed: {e}");
                return;
            }
        };
        if update != sent {
            if socket.send(Message::text(update.clone())).await.is_err() {
                return;
            }
            sent = update;
        }
        tokio::select! {
            changed = lifecycle.changed() => if changed.is_err() { return },
            changed = watching.listed.changed() => if changed.is_err() { return },
            received = socket.recv() => if hung_up(received) { return },
        }
    }
}

#[derive(Deserialize)]
pub struct Pair {
    paths: [String; 2],
}

#[derive(Serialize)]
struct Compared {
    changed: Option<u32>,
    boxes: Vec<CellBox>,
}

pub async fn compare(
    State(backend): State<Arc<dyn Backend>>,
    Path(session): Path<String>,
    Json(Pair { paths }): Json<Pair>,
) -> Response {
    let Some(folder) = backend.folder(&session) else {
        return not_listed(&paths[0], &session);
    };
    match tokio::task::spawn_blocking(move || compared(&folder, &session, &paths)).await {
        Ok(response) => response,
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("comparing the screenshots failed: {e}")).into_response(),
    }
}

fn compared(folder: &SessionFolder, session: &str, paths: &[String; 2]) -> Response {
    let listed = match folder.screenshots() {
        Ok(listed) => listed,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")).into_response(),
    };
    let find = |path: &String| listed.iter().find(|shot| shot.path.as_os_str() == path.as_str());
    let (older, newer) = match (find(&paths[0]), find(&paths[1])) {
        (Some(older), Some(newer)) => (older, newer),
        (None, _) => return not_listed(&paths[0], session),
        (_, None) => return not_listed(&paths[1], session),
    };
    if older.cells != newer.cells {
        return Json(Compared { changed: None, boxes: Vec::new() }).into_response();
    }
    let read = |shot: &ScreenshotEntry| {
        std::fs::read(&shot.path).with_context(|| format!("cannot read the screenshot {}", shot.path.display()))
    };
    let changed = read(older)
        .and_then(|older_png| Ok((older_png, read(newer)?)))
        .and_then(|(older_png, newer_png)| render::changed_cells(&older_png, &newer_png, older.cells));
    match changed {
        Ok(changed) => Json(Compared { changed: Some(changed.count), boxes: changed.boxes }).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, format!("{e:#}")).into_response(),
    }
}

fn not_listed(path: &str, session: &str) -> Response {
    (StatusCode::NOT_FOUND, format!("{path} is not a PNG screenshot in session {session}")).into_response()
}
