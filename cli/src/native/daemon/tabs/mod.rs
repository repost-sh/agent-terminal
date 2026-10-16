use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant};

use super::refusals::{error_body, no_session, no_tab, no_terminal};
use super::waiting::{Until, condition};
use super::{Daemon, Session, Tab};
use crate::native::control::Control;
use crate::native::log;
use crate::native::pty;
use crate::native::shell;
use crate::native::terminal::{HANG_UP_GRACE, Labelled, Settled, Starting, Terminal};
use crate::native::wait::Condition;
use crate::protocol::{Closed, ErrorBody, Name, Reply, SessionInfo, Shell, TabId, TabRef, Target, Wait, request};

const KILLED_WITHIN: Duration = Duration::from_secs(5);

impl Daemon {
    pub(super) fn open(&self, open: request::Open) -> Result<Reply, ErrorBody> {
        let request::Open { session, program, args, size, cwd, env, wait, timeout_ms } = open;
        let (tab, terminal) = self.open_tab(&session, pty::Command { program, args, cwd, env, size }, None)?;
        first_screen(&session, tab, &terminal, wait.as_ref(), timeout_ms)
    }

    pub(super) fn open_shell(&self, opening: request::OpenShell) -> Result<Reply, ErrorBody> {
        let request::OpenShell { session, shell: kind, program, size, cwd, env, wait, timeout_ms } = opening;
        let command = pty::Command { program, args: Vec::new(), cwd, env, size };
        let command = shell::Scripts { home: &self.home }.prepare(kind, command).map_err(|e| error_body(&e))?;
        let (tab, terminal) = self.open_tab(&session, command, Some(kind))?;
        first_screen(&session, tab, &terminal, wait.as_ref(), timeout_ms).map_err(|mut error| {
            if matches!(wait, Some(Wait::Command)) && error.code == crate::exit::TIMEOUT {
                error.message = format!(
                    "timed out after {timeout_ms} ms waiting for the first prompt of {} in {session} {tab}; its startup files may be waiting for input or the shell integration did not load (read the screen with `agent-terminal snapshot`, check with `agent-terminal doctor`)",
                    terminal.program()
                );
            }
            error
        })
    }

    fn open_tab(
        &self,
        session: &Name,
        command: pty::Command,
        shell: Option<Shell>,
    ) -> Result<(TabId, Arc<Terminal>), ErrorBody> {
        let folder = self.folder_of(session)?;
        let spawned = pty::spawn(&command).map_err(|e| error_body(&e))?;
        let tab = folder.next_tab();
        let command_log = folder.command_log(&tab.to_string());
        let terminal = Terminal::start(Starting {
            command,
            shell,
            spawned,
            command_log,
            lifecycle: self.changes.clone(),
            continued: None,
        });
        {
            let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(entry) = sessions.get_mut(session) else {
                drop(sessions);
                stop_all(std::slice::from_ref(&terminal));
                return Err(no_session(session));
            };
            let control = Control::new(self.changes.clone());
            entry.tabs.insert(tab, Tab { terminal: Arc::clone(&terminal), control });
            entry.current = Some(tab);
        }
        self.changes.notify();
        Ok((tab, terminal))
    }

    pub(super) fn resize(&self, resize: &request::Resize) -> Result<Reply, ErrorBody> {
        let (tab, terminal) = self.target(&resize.target)?;
        let session = &resize.target.session;
        terminal.resize(resize.size).map_err(|e| error_body(&e.context(format!("{session} {tab}"))))?;
        settle(session, tab, &terminal, &resize.wait, resize.timeout_ms)
    }

    pub(super) fn restart(&self, restart: &request::Restart) -> Result<Reply, ErrorBody> {
        let restarting = self.restarting.lock().unwrap_or_else(PoisonError::into_inner);
        let (tab, old) = self.target(&restart.target)?;
        let session = &restart.target.session;
        let timeout_ms = restart.timeout_ms;
        if !old.stop(Duration::from_millis(timeout_ms)) {
            return Err(ErrorBody::new(
                crate::exit::TIMEOUT,
                format!(
                    "{} in {session} {tab} did not stop within {timeout_ms} ms, so it was not restarted",
                    old.program()
                ),
            ));
        }
        let command = old.rerun();
        let spawned = pty::spawn(&command).map_err(|e| error_body(&e))?;
        let continued = old.continued();
        let terminal = Terminal::start(Starting {
            command,
            shell: old.shell(),
            spawned,
            command_log: old.command_log(),
            lifecycle: self.changes.clone(),
            continued: Some(continued),
        });
        self.replace(session, tab, &old, &terminal)?;
        self.changes.notify();
        drop(restarting);
        first_screen(session, tab, &terminal, Some(&restart.wait), timeout_ms)
    }

    fn replace(&self, session: &str, tab: TabId, old: &Arc<Terminal>, new: &Arc<Terminal>) -> Result<(), ErrorBody> {
        let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        match sessions.get_mut(session).and_then(|s| s.tabs.get_mut(&tab)) {
            Some(slot) if Arc::ptr_eq(&slot.terminal, old) => {
                slot.terminal = Arc::clone(new);
                Ok(())
            }
            _ => {
                drop(sessions);
                stop_all(std::slice::from_ref(new));
                Err(no_tab(session, tab))
            }
        }
    }

    pub(super) fn tabs_of(&self, session: &Name) -> Result<SessionInfo, ErrorBody> {
        let sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        sessions.get(session).map(|s| s.info(session)).ok_or_else(|| no_session(session))
    }

    pub(super) fn switch_tab(&self, switch: &request::SwitchTab) -> Result<Reply, ErrorBody> {
        let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = sessions.get_mut(&switch.session).ok_or_else(|| no_session(&switch.session))?;
        if !entry.tabs.contains_key(&switch.tab) {
            return Err(no_tab(&switch.session, switch.tab));
        }
        entry.current = Some(switch.tab);
        Ok(Reply::Tabs(entry.info(&switch.session)))
    }

    pub(super) fn close_request(&self, close: &request::Close) -> Result<Reply, ErrorBody> {
        let closed = match close {
            request::Close::Tab(target) => Closed { closed: vec![self.close(target)?], ended: Vec::new() },
            request::Close::Session { session } => self.close_session(session)?,
            request::Close::EverySession => self.close_all(),
        };
        Ok(Reply::Closed(closed))
    }

    fn close(&self, target: &Target) -> Result<TabRef, ErrorBody> {
        let Target { session, tab } = target;
        let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let missing = || tab.map_or_else(|| no_terminal(session), |id| no_tab(session, id));
        let entry = sessions.get_mut(session).ok_or_else(|| no_session(session))?;
        let id = tab.or(entry.current).ok_or_else(missing)?;
        let closing = entry.tabs.remove(&id).ok_or_else(missing)?;
        if entry.current == Some(id) {
            entry.current = entry.tabs.keys().next_back().copied();
        }
        drop(sessions);
        self.changes.notify();
        closing.control.close();
        stop_all(&[closing.terminal]);
        Ok(TabRef { session: session.to_string(), tab: id.to_string() })
    }

    fn close_session(&self, name: &Name) -> Result<Closed, ErrorBody> {
        let session = self.sessions.lock().unwrap_or_else(PoisonError::into_inner).remove(name);
        let session = session.ok_or_else(|| no_session(name))?;
        Ok(self.close_sessions([(name.clone(), session)]))
    }

    pub(super) fn close_all(&self) -> Closed {
        let sessions = std::mem::take(&mut *self.sessions.lock().unwrap_or_else(PoisonError::into_inner));
        self.close_sessions(sessions)
    }

    fn close_sessions(&self, sessions: impl IntoIterator<Item = (Name, Session)>) -> Closed {
        self.changes.notify();
        let mut closed = Vec::new();
        let mut ended = Vec::new();
        let mut terminals = Vec::new();
        for (name, session) in sessions {
            for (id, closing) in session.tabs {
                closing.control.close();
                terminals.push(closing.terminal);
                closed.push(TabRef { session: name.to_string(), tab: id.to_string() });
            }
            ended.push(name);
        }
        stop_all(&terminals);
        Closed { closed, ended }
    }
}

fn stop_all(terminals: &[Arc<Terminal>]) {
    for terminal in terminals {
        terminal.hang_up();
    }
    let grace = Instant::now() + HANG_UP_GRACE;
    let holding: Vec<&Arc<Terminal>> = terminals
        .iter()
        .filter(|terminal| !terminal.exited_within(grace.saturating_duration_since(Instant::now())))
        .collect();
    for terminal in &holding {
        terminal.kill();
    }
    let deadline = Instant::now() + KILLED_WITHIN;
    for terminal in holding {
        if !terminal.exited_within(deadline.saturating_duration_since(Instant::now())) {
            log::note(format_args!(
                "{}: still running {} ms after SIGKILL",
                terminal.program(),
                KILLED_WITHIN.as_millis()
            ));
        }
    }
}

fn first_screen(
    session: &str,
    tab: TabId,
    terminal: &Terminal,
    wait: Option<&Wait>,
    timeout_ms: u64,
) -> Result<Reply, ErrorBody> {
    let tab = tab.to_string();
    let at = Labelled { session, tab: &tab, terminal };
    match wait {
        Some(wait) => {
            let launch = condition(wait)?.after_launch(Instant::now(), Duration::from_millis(timeout_ms));
            settled_on(at, wait, launch, timeout_ms)
        }
        None => Ok(Reply::Snapshot(at.snapshot())),
    }
}

fn settle(session: &str, tab: TabId, terminal: &Terminal, wait: &Wait, timeout_ms: u64) -> Result<Reply, ErrorBody> {
    let action = condition(wait)?.counted_from(Instant::now());
    settled_on(Labelled { session, tab: &tab.to_string(), terminal }, wait, action, timeout_ms)
}

fn settled_on(at: Labelled<'_>, wait: &Wait, condition: Condition, timeout_ms: u64) -> Result<Reply, ErrorBody> {
    let settled = at.terminal.wait(condition, Duration::from_millis(timeout_ms));
    let snapshot = at.snapshot();
    match settled {
        Settled::Met { .. } | Settled::Exited => Ok(Reply::Snapshot(snapshot)),
        Settled::TimedOut => Err(Until { wait, timeout_ms }.timed_out(snapshot)),
    }
}

#[cfg(test)]
mod tests;
