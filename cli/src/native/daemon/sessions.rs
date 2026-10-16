use std::sync::{Arc, PoisonError, Weak};

use anyhow::Result;

use super::refusals::{error_body, no_session, taken};
use super::{Daemon, Session};
use crate::native::store::SessionFolder;
use crate::protocol::{Created, ErrorBody, Reply, request};

impl Daemon {
    pub(super) fn new_session(&self, new: request::NewSession) -> Result<Reply, ErrorBody> {
        let request::NewSession { session } = new;
        let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(held) = sessions.get(&session) {
            return Err(taken(&held.info(&session), held.created.elapsed()));
        }
        let folder = self.session_folder(&session).map_err(|e| error_body(&e))?;
        sessions.insert(session.clone(), Session::new(folder));
        drop(sessions);
        self.changes.notify();
        Ok(Reply::Created(Created { session }))
    }

    pub(super) fn folder_of(&self, session: &str) -> Result<Arc<SessionFolder>, ErrorBody> {
        let sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        sessions.get(session).map(|entry| Arc::clone(&entry.folder)).ok_or_else(|| no_session(session))
    }

    fn session_folder(&self, session: &str) -> Result<Arc<SessionFolder>> {
        let mut folders = self.folders.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(folder) = folders.get(session).and_then(Weak::upgrade) {
            return Ok(folder);
        }
        let folder = Arc::new(SessionFolder::start(&self.home, session)?);
        folders.retain(|_, folder| folder.strong_count() > 0);
        folders.insert(session.to_string(), Arc::downgrade(&folder));
        Ok(folder)
    }
}
