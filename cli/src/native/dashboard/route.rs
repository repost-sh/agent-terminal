use std::sync::Arc;

use axum::extract::{FromRequestParts, Path};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};

use super::Backend;
use crate::protocol::TabId;

pub(super) struct SessionRoute {
    pub backend: Arc<dyn Backend>,
    pub session: String,
}

pub(super) struct TabRoute {
    pub backend: Arc<dyn Backend>,
    pub session: String,
    pub id: TabId,
}

impl FromRequestParts<Arc<dyn Backend>> for SessionRoute {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<dyn Backend>) -> Result<Self, Response> {
        let Path(session) =
            Path::<String>::from_request_parts(parts, state).await.map_err(IntoResponse::into_response)?;
        Ok(Self { backend: Arc::clone(state), session })
    }
}

impl FromRequestParts<Arc<dyn Backend>> for TabRoute {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<dyn Backend>) -> Result<Self, Response> {
        let Path((session, tab)) =
            Path::<(String, String)>::from_request_parts(parts, state).await.map_err(IntoResponse::into_response)?;
        match tab.parse() {
            Ok(id) => Ok(Self { backend: Arc::clone(state), session, id }),
            Err(_) => Err(not_a_tab(&tab)),
        }
    }
}

pub(super) fn not_a_tab(tab: &str) -> Response {
    (StatusCode::NOT_FOUND, format!("{tab} is not a tab id; tabs are t1, t2, ...")).into_response()
}
