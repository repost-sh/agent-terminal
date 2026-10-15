use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::header::{HOST, ORIGIN};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

const LOOPBACK: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];

pub struct Pages {
    pub port: u16,
    pub origins: Vec<String>,
}

pub async fn same_origin(State(pages): State<Arc<Pages>>, request: Request, next: Next) -> Response {
    if from_our_pages(request.headers(), &pages) {
        return next.run(request).await;
    }
    (StatusCode::FORBIDDEN, format!("the dashboard only answers its own pages at http://localhost:{}", pages.port))
        .into_response()
}

fn from_our_pages(headers: &HeaderMap, pages: &Pages) -> bool {
    let ours = |authority: &str| LOOPBACK.iter().any(|host| authority == format!("{host}:{}", pages.port));
    let header = |name| headers.get(name).map(|value| value.to_str().unwrap_or_default());
    let host_is_ours = header(HOST).is_some_and(ours);
    let origin_is_ours = header(ORIGIN).is_none_or(|origin| {
        origin.strip_prefix("http://").is_some_and(ours) || pages.origins.iter().any(|allowed| allowed == origin)
    });
    host_is_ours && origin_is_ours
}
