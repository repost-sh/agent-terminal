use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use super::assets;
use crate::native::render::fonts::Face;

pub(super) async fn serve(Path(name): Path<String>) -> Response {
    match Face::ALL.into_iter().find(|face| face.file_name() == name) {
        Some(face) => assets::respond("ttf", assets::IMMUTABLE, face.bytes()),
        None => (StatusCode::NOT_FOUND, "no such font").into_response(),
    }
}
