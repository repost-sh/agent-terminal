use std::sync::{Arc, PoisonError};
use std::time::{Instant, SystemTime};

use super::Daemon;
use crate::native::store::{ActivityEntry, Pending, SessionFolder, TabId, Typed};
use crate::output;
use crate::protocol::{Addressed, ErrorBody, Input, Name, Reply, Request, request};

pub(super) struct Logged {
    session: Name,
    tab: Option<(TabId, String)>,
    raw: bool,
    folder: Option<Arc<SessionFolder>>,
    pending: Option<Pending>,
    pub(super) typed: Vec<String>,
    time: SystemTime,
    started: Instant,
}

impl Daemon {
    pub(super) fn about_to_handle(&self, typed: Vec<String>, request: &Request) -> Option<Logged> {
        let sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let (session, tab) = match request.addressed() {
            Addressed::Nothing => return None,
            Addressed::Session(session) => (session, None),
            Addressed::Tab(session, tab) => {
                let found = sessions.get(session).and_then(|s| s.find(session, tab.map(|name| &**name)).ok());
                (session, found.map(|(id, tab)| (id, tab.name.to_string())))
            }
        };
        let time = SystemTime::now();
        let folder = sessions.get(session).map(|s| Arc::clone(&s.folder));
        let pending = folder.as_ref().map(|folder| folder.begin(&Typed { time, words: &typed }, tab.clone()));
        Some(Logged {
            session: session.clone(),
            tab,
            raw: matches!(request, Request::Input(request::Deliver { input: Input::Send { .. }, .. })),
            folder,
            pending,
            typed,
            time,
            started: Instant::now(),
        })
    }

    pub(super) fn log(&self, logged: Logged, handled: &Result<Reply, ErrorBody>) {
        let folder = logged.folder.clone().or_else(|| {
            self.sessions
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(&logged.session)
                .map(|s| Arc::clone(&s.folder))
        });
        let Some(folder) = folder else {
            return;
        };
        let entry = self.entry(&logged, handled);
        match logged.pending {
            Some(pending) => pending.log(&entry),
            None => folder.log(&entry),
        }
    }

    fn entry(&self, logged: &Logged, handled: &Result<Reply, ErrorBody>) -> ActivityEntry {
        let mut entry = ActivityEntry::at(&Typed { time: logged.time, words: &logged.typed }, logged.started.elapsed());
        entry.raw = logged.raw;
        let shown = match handled {
            Ok(Reply::Snapshot(snapshot)) => Some(snapshot),
            Err(failure) => failure.snapshot.as_deref(),
            Ok(_) => None,
        };
        let opened = shown.and_then(|snapshot| self.id_of(&logged.session, &snapshot.tab));
        entry.tab_id = logged.tab.as_ref().map(|(id, _)| *id).or(opened);
        entry.tab = entry
            .tab_id
            .and_then(|id| self.name_of(&logged.session, id))
            .map(String::from)
            .or_else(|| shown.map(|snapshot| snapshot.tab.clone()))
            .or_else(|| logged.tab.as_ref().map(|(_, name)| name.clone()));
        entry.frame = shown
            .map(|snapshot| snapshot.frame)
            .or_else(|| self.terminal_of(&logged.session, entry.tab_id?).map(|terminal| terminal.frame()));
        record_outcome(&mut entry, handled);
        entry.screenshots = screenshots(handled);
        entry
    }
}

fn record_outcome(entry: &mut ActivityEntry, handled: &Result<Reply, ErrorBody>) {
    match handled {
        Ok(reply) => {
            entry.exit = reply.exit_code();
            entry.printed(output::reply_text(reply));
        }
        Err(failure) => {
            entry.exit = failure.code;
            entry.error = Some(failure.message.clone());
            entry.printed(output::error_text(failure));
        }
    }
    if let Ok(Reply::ScreenshotDiff(diff)) = handled
        && !diff.passed
    {
        entry.error = Some(diff.summary());
    }
}

fn screenshots(handled: &Result<Reply, ErrorBody>) -> Vec<std::path::PathBuf> {
    match handled {
        Ok(Reply::Screenshot(screenshot)) => vec![screenshot.path.clone()],
        Ok(Reply::ScreenshotDiff(diff)) => vec![diff.path.clone()],
        Ok(Reply::Delivered(delivered)) => delivered.screenshot.iter().map(|shot| shot.path.clone()).collect(),
        Ok(_) | Err(_) => Vec::new(),
    }
}
