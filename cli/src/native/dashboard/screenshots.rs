use std::sync::Arc;

use super::follow::Follow;
use super::route::SessionRoute;
use super::stream::{LastSent, send};
use crate::native::log;
use crate::native::render::CellBox;
use crate::native::store::{ScreenshotEntry, SessionFolder};
use anyhow::Context;
use axum::Json;
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum Update {
    Screenshots { screenshots: Vec<ScreenshotEntry> },
    Closed,
}

pub(super) async fn stream(ws: WebSocketUpgrade, route: SessionRoute) -> Response {
    ws.on_upgrade(move |socket| follow_screenshots(socket, route))
}

async fn follow_screenshots(mut socket: WebSocket, route: SessionRoute) {
    let SessionRoute { backend, session } = route;
    let looking = Arc::clone(&backend);
    let named = session.clone();
    let lookup = Box::new(move || looking.folder(&named));
    let mut follow =
        Follow::new(backend.changes(), lookup).signal(|folder: &SessionFolder| Some(folder.screenshots_listed()));
    let mut sent = LastSent::default();
    loop {
        let Some((folder, _)) = follow.next() else {
            send(&mut socket, &Update::Closed).await;
            return;
        };
        let Some(update) = screenshots_update(folder, &session).await else {
            return;
        };
        if !sent.send(&mut socket, update).await || !follow.changed(&mut socket).await {
            return;
        }
    }
}

async fn screenshots_update(folder: Arc<SessionFolder>, session: &str) -> Option<String> {
    let screenshots = match tokio::task::spawn_blocking(move || folder.screenshots()).await {
        Ok(Ok(screenshots)) => screenshots,
        Ok(Err(e)) => {
            log::note(format_args!("dashboard: reading the screenshots of {session} failed: {e:#}"));
            return None;
        }
        Err(e) => {
            log::note(format_args!("dashboard: reading the screenshots of {session} failed: {e}"));
            return None;
        }
    };
    serde_json::to_string(&Update::Screenshots { screenshots })
        .inspect_err(|e| log::note(format_args!("dashboard: encoding the screenshots of {session} failed: {e}")))
        .ok()
}

#[derive(Deserialize)]
pub(super) struct Pair {
    paths: [String; 2],
}

#[derive(Serialize)]
struct Compared {
    changed: Option<u32>,
    boxes: Vec<CellBox>,
}

pub(super) async fn compare(route: SessionRoute, Json(Pair { paths }): Json<Pair>) -> Response {
    let SessionRoute { backend, session } = route;
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
        .and_then(|(older_png, newer_png)| older.cells.changed_between(&older_png, &newer_png));
    match changed {
        Ok(changed) => Json(Compared { changed: Some(changed.count), boxes: changed.boxes }).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, format!("{e:#}")).into_response(),
    }
}

fn not_listed(path: &str, session: &str) -> Response {
    (StatusCode::NOT_FOUND, format!("{path} is not a PNG screenshot in session {session}")).into_response()
}
