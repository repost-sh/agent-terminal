use axum::extract::{Request, State};
use axum::http::header::{HOST, ORIGIN};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

const LOOPBACK: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];

pub async fn same_origin(State(port): State<u16>, request: Request, next: Next) -> Response {
    if from_our_pages(request.headers(), port) {
        return next.run(request).await;
    }
    (StatusCode::FORBIDDEN, format!("the dashboard only answers its own pages at http://localhost:{port}"))
        .into_response()
}

fn from_our_pages(headers: &HeaderMap, port: u16) -> bool {
    let ours = |authority: &str| LOOPBACK.iter().any(|host| authority == format!("{host}:{port}"));
    let header = |name| headers.get(name).map(|value| value.to_str().unwrap_or_default());
    let host_is_ours = header(HOST).is_some_and(ours);
    let origin_is_ours = header(ORIGIN).is_none_or(|origin| origin.strip_prefix("http://").is_some_and(ours));
    host_is_ours && origin_is_ours
}
