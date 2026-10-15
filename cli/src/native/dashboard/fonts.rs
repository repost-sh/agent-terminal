use axum::extract::Path;
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};

use crate::native::render::fonts::Face;

pub async fn serve(Path(name): Path<String>) -> Response {
    match Face::ALL.into_iter().find(|face| face.file_name() == name) {
        Some(face) => {
            ([(CONTENT_TYPE, "font/ttf"), (CACHE_CONTROL, "public, max-age=31536000, immutable")], face.bytes())
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "no such font").into_response(),
    }
}
