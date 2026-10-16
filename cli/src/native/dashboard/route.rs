use std::sync::Arc;

use axum::extract::{FromRequestParts, Path};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};

use super::Backend;
use crate::native::store::TabId;
use crate::protocol::Name;

pub(super) struct SessionRoute {
    pub backend: Arc<dyn Backend>,
    pub session: String,
}

pub(super) struct TabRoute {
    pub backend: Arc<dyn Backend>,
    pub session: String,
    pub name: Name,
    pub id: Option<TabId>,
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
        let name: Name = tab.parse().map_err(|refused: String| (StatusCode::NOT_FOUND, refused).into_response())?;
        let id = state.tab_id(&session, &name);
        Ok(Self { backend: Arc::clone(state), session, name, id })
    }
}
