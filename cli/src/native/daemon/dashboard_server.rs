use std::sync::{Arc, PoisonError};

use super::Daemon;
use super::refusals::error_body;
use crate::native::dashboard::{self, Backend};
use crate::protocol::{Dashboard, ErrorBody, request};

impl Daemon {
    pub(super) fn start_dashboard(&self, start: request::DashboardStart) -> Result<Dashboard, ErrorBody> {
        let request::DashboardStart { port, origins } = start;
        let mut running = self.dashboard.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(server) = running.as_ref() {
            if (port == 0 || port == server.port()) && origins.iter().all(|origin| server.allows(origin)) {
                return Ok(server.info());
            }
            return Err(ErrorBody::failure(format!(
                "the dashboard already runs at {}; stop it first with `agent-terminal dashboard stop`",
                server.info().url
            )));
        }
        let backend: Arc<dyn Backend> =
            self.me.upgrade().ok_or_else(|| ErrorBody::failure("the daemon is shutting down"))?;
        let server = dashboard::Server::start(port, origins, backend).map_err(|e| error_body(&e))?;
        let info = server.info();
        *running = Some(server);
        Ok(info)
    }

    pub(super) fn stop_dashboard(&self) -> bool {
        let server = self.dashboard.lock().unwrap_or_else(PoisonError::into_inner).take();
        server.is_some()
    }
}
