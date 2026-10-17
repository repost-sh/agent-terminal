use std::sync::PoisonError;
use std::time::{Instant, SystemTime};

use super::Daemon;
use crate::native::store::{ActivityEntry, Pending, TabId, Typed};
use crate::output;
use crate::protocol::{Addressed, ErrorBody, Input, Name, Reply, Request, Squeeze, request};

pub(super) struct Logged {
    session: Name,
    tab: Option<(TabId, Name)>,
    opens: Option<Name>,
    raw: bool,
    squeeze: Squeeze,
    pending: Option<Pending>,
    pub(super) typed: Vec<String>,
    action: Option<String>,
    time: SystemTime,
    started: Instant,
}

impl Daemon {
    pub(super) fn about_to_handle(&self, typed: Vec<String>, request: &Request) -> Option<Logged> {
        let sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let (session, tab, opens) = match request.addressed() {
            Addressed::Nothing => return None,
            Addressed::Session(session) => (session, None, None),
            Addressed::Opens(session, tab) => (session, None, Some(tab.clone())),
            Addressed::Tab(session, tab) => {
                let found = sessions.get(session).and_then(|s| s.find(session, tab.map(|name| &**name)).ok());
                (session, found.map(|(id, tab)| (id, tab.name.clone())), None)
            }
        };
        let time = SystemTime::now();
        let action = output::action(request);
        let called = Typed { time, words: &typed, action: action.as_deref() };
        let pending = sessions.get(session).map(|s| s.folder.begin(&called, tab.clone()));
        Some(Logged {
            session: session.clone(),
            tab,
            opens,
            raw: matches!(request, Request::Input(request::Deliver { input: Input::Send { .. }, .. })),
            squeeze: request.squeeze(),
            pending,
            typed,
            action,
            time,
            started: Instant::now(),
        })
    }

    pub(super) fn log(&self, mut logged: Logged, handled: &Result<Reply, ErrorBody>) {
        if let Some(pending) = logged.pending.take() {
            pending.log(&self.entry(&logged, handled));
        }
    }

    fn entry(&self, logged: &Logged, handled: &Result<Reply, ErrorBody>) -> ActivityEntry {
        let mut entry = ActivityEntry::at(
            &Typed { time: logged.time, words: &logged.typed, action: logged.action.as_deref() },
            logged.started.elapsed(),
        );
        entry.raw = logged.raw;
        let shown = match handled {
            Ok(Reply::Snapshot(snapshot)) => Some(snapshot),
            Err(failure) => failure.snapshot.as_deref(),
            Ok(_) => None,
        };
        let now = self.logged_tab_now(logged, shown.is_some());
        entry.tab_id = now.as_ref().map(|(id, ..)| *id).or(logged.tab.as_ref().map(|(id, _)| *id));
        entry.tab = now
            .as_ref()
            .map(|(_, name, _)| name.clone())
            .or_else(|| shown.map(|snapshot| snapshot.tab.clone()))
            .or_else(|| logged.tab.as_ref().map(|(_, name)| name.clone()));
        entry.frame = shown.map(|snapshot| snapshot.frame).or(now.map(|(.., frame)| frame));
        record_outcome(&mut entry, handled, &logged.squeeze);
        entry.screenshots = screenshots(handled);
        entry
    }

    fn logged_tab_now(&self, logged: &Logged, opened: bool) -> Option<(TabId, Name, u64)> {
        let sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let session = sessions.get(&logged.session)?;
        let (id, tab) = match (&logged.tab, &logged.opens) {
            (Some((id, _)), _) => session.tabs.get_key_value(id)?,
            (None, Some(name)) if opened => session.named(name)?,
            (None, _) => return None,
        };
        Some((*id, tab.name.clone(), tab.terminal.frame()))
    }
}

fn record_outcome(entry: &mut ActivityEntry, handled: &Result<Reply, ErrorBody>, squeeze: &Squeeze) {
    match handled {
        Ok(reply) => {
            entry.exit = reply.exit_code();
            entry.printed(output::reply_text(reply, squeeze));
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
