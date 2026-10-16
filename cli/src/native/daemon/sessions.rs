use std::collections::BTreeMap;
use std::sync::{Arc, PoisonError, Weak};

use anyhow::Result;

use super::refusals::{error_body, taken};
use super::{Daemon, Session};
use crate::native::store::SessionFolder;
use crate::protocol::{Created, ErrorBody, Name, Reply, request};

impl Daemon {
    pub(super) fn new_session(&self, new: request::NewSession) -> Result<Reply, ErrorBody> {
        let request::NewSession { session } = new;
        let folder = self.free_session_folder(&session)?;
        self.add_session(&session, Session::new(folder))?;
        self.changes.notify();
        Ok(Reply::Created(Created { session }))
    }

    pub(super) fn free_session_folder(&self, session: &Name) -> Result<Arc<SessionFolder>, ErrorBody> {
        free(&self.sessions.lock().unwrap_or_else(PoisonError::into_inner), session)?;
        self.session_folder(session).map_err(|e| error_body(&e))
    }

    pub(super) fn add_session(&self, session: &Name, added: Session) -> Result<(), ErrorBody> {
        let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        free(&sessions, session)?;
        sessions.insert(session.clone(), added);
        Ok(())
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

fn free(sessions: &BTreeMap<Name, Session>, session: &Name) -> Result<(), ErrorBody> {
    match sessions.get(session) {
        Some(held) => Err(taken(session, held)),
        None => Ok(()),
    }
}
