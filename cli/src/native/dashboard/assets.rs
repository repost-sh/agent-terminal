use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use include_dir::{Dir, File, include_dir};

static WEB_APP: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/assets/dashboard");

pub(super) const IMMUTABLE: &str = "public, max-age=31536000, immutable";

pub(super) async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path == "api" || path.starts_with("api/") {
        return (StatusCode::NOT_FOUND, "no such dashboard API").into_response();
    }
    if let Some(file) = WEB_APP.get_file(path) {
        let cache = if path.starts_with("assets/") { IMMUTABLE } else { "no-cache" };
        return send(file, cache);
    }
    app_route(path)
}

fn app_route(path: &str) -> Response {
    let names_a_file = path.rsplit('/').next().is_some_and(|last| last.contains('.'));
    match WEB_APP.get_file("index.html") {
        Some(page) if !names_a_file => send(page, "no-cache"),
        _ => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

fn send(file: &'static File<'static>, cache: &'static str) -> Response {
    let extension = file.path().extension().and_then(|e| e.to_str()).unwrap_or_default();
    respond(extension, cache, file.contents())
}

pub(super) fn respond(extension: &str, cache: &'static str, bytes: &'static [u8]) -> Response {
    ([(CONTENT_TYPE, content_type(extension)), (CACHE_CONTROL, cache)], bytes).into_response()
}

fn content_type(extension: &str) -> &'static str {
    match extension {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
