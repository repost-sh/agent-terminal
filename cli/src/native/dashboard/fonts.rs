use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use super::assets::{self, Asset};
use crate::native::render::fonts::Face;

pub(super) async fn serve(Path(name): Path<String>) -> Response {
    match Face::ALL.into_iter().find(|face| face.file_name() == name) {
        Some(face) => Asset { extension: "ttf", cache: assets::IMMUTABLE, bytes: face.bytes() }.into_response(),
        None => (StatusCode::NOT_FOUND, "no such font").into_response(),
    }
}
