mod activity;
mod attach;
mod batch;
mod claim;
mod dashboard_server;
mod dispatch;
mod expect;
mod input;
mod lifecycle;
mod reading;
mod refusals;
mod serve;
mod sessions;
mod tabs;
mod waiting;

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{self, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Condvar, Mutex, PoisonError, Weak};
use std::time::{Instant, SystemTime};

use anyhow::{Context, Result};
use tokio::sync::watch;

use crate::home::home;
use crate::native::changes::Changes;
use crate::native::control::{Control, TabHold};
use crate::native::dashboard::{self, Backend};
use crate::native::log;
use crate::native::store::{ActivityEntry, SessionFolder, Typed};
use crate::native::terminal::Terminal;
use crate::protocol::{ErrorBody, HumanInput, Name, Reply, Request, SessionInfo, Sessions, TabId, Target};
use lifecycle::Activity;
use refusals::{error_body, no_session, no_tab, no_terminal, stopping};

pub(crate) const READY: &str = "ready";

struct Daemon {
    home: PathBuf,
    claim: File,
    sessions: Mutex<BTreeMap<Name, Session>>,
    restarting: Mutex<()>,
    activity: Mutex<Activity>,
    quiet: Condvar,
    folders: Mutex<HashMap<String, Weak<SessionFolder>>>,
    changes: Changes,
    dashboard: Mutex<Option<dashboard::Server>>,
    me: Weak<Daemon>,
    ended: Sender<()>,
}

struct Session {
    tabs: BTreeMap<TabId, Tab>,
    current: Option<TabId>,
    folder: Arc<SessionFolder>,
    created: Instant,
}

#[derive(Clone)]
struct Tab {
    terminal: Arc<Terminal>,
    control: Arc<Control>,
}

pub(crate) fn run() -> Result<()> {
    log::start()?;
    let home = home()?;
    std::fs::create_dir_all(&home).with_context(|| format!("create {}", home.display()))?;
    let (claim, listener) = claim::claim(&home)?;

    let mut stdout = io::stdout();
    writeln!(stdout, "{READY}").and_then(|()| stdout.flush()).context("tell the CLI the daemon is ready")?;

    let (ended, end) = mpsc::channel();
    let daemon = Arc::new_cyclic(|me| Daemon::new(Weak::clone(me), ended, home, claim));
    let accepting = Arc::clone(&daemon);
    std::thread::spawn(move || accepting.accept(&listener));
    end.recv().context("every thread of the daemon ended without stopping it")
}

impl Daemon {
    fn new(me: Weak<Daemon>, ended: Sender<()>, home: PathBuf, claim: File) -> Self {
        Self {
            home,
            claim,
            sessions: Mutex::default(),
            restarting: Mutex::default(),
            activity: Mutex::default(),
            quiet: Condvar::new(),
            folders: Mutex::default(),
            changes: Changes::default(),
            dashboard: Mutex::default(),
            me,
            ended,
        }
    }

    fn accept(self: &Arc<Self>, listener: &UnixListener) {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let daemon = Arc::clone(self);
                    std::thread::spawn(move || daemon.serve(stream));
                }
                Err(e) => log::note(format_args!("accepting a connection failed: {e}")),
            }
        }
    }

    fn end(&self) {
        if let Err(e) = self.ended.send(()) {
            log::note(format_args!("ending the daemon failed: {e}"));
        }
    }

    fn list(&self) -> Sessions {
        let sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        Sessions { sessions: sessions.iter().map(|(name, session)| session.info(name)).collect() }
    }

    fn terminals(&self) -> usize {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner).values().map(|session| session.tabs.len()).sum()
    }

    fn terminal_of(&self, session: &str, id: TabId) -> Option<Arc<Terminal>> {
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(session)
            .and_then(|entry| entry.tabs.get(&id))
            .map(|tab| Arc::clone(&tab.terminal))
    }

    fn terminal_under(&self, session: &str, id: TabId, control: &Arc<Control>) -> Option<Arc<Terminal>> {
        let sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let tab = sessions.get(session).and_then(|entry| entry.tabs.get(&id))?;
        Arc::ptr_eq(&tab.control, control).then(|| Arc::clone(&tab.terminal))
    }

    fn target(&self, target: &Target) -> Result<(TabId, Arc<Terminal>), ErrorBody> {
        self.tab(target).map(|(id, tab)| (id, tab.terminal))
    }

    fn tab(&self, target: &Target) -> Result<(TabId, Tab), ErrorBody> {
        self.tab_in(&target.session, target.tab)
    }

    fn tab_in(&self, session: &str, tab: Option<TabId>) -> Result<(TabId, Tab), ErrorBody> {
        let sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = sessions.get(session).ok_or_else(|| no_session(session))?;
        match tab {
            Some(id) => entry.tabs.get(&id).map(|t| (id, t.clone())).ok_or_else(|| no_tab(session, id)),
            None => entry
                .current
                .and_then(|id| entry.tabs.get(&id).map(|t| (id, t.clone())))
                .ok_or_else(|| no_terminal(session)),
        }
    }
}

impl Backend for Daemon {
    fn call(&self, request: Request) -> Result<Reply, ErrorBody> {
        match self.busy() {
            Some(_busy) => self.handle(request, None),
            None => Err(stopping()),
        }
    }

    fn sessions(&self) -> Sessions {
        self.list()
    }

    fn terminal(&self, session: &str, tab: TabId) -> Option<Arc<Terminal>> {
        self.terminal_of(session, tab)
    }

    fn folder(&self, session: &str) -> Option<Arc<SessionFolder>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner).get(session).map(|entry| Arc::clone(&entry.folder))
    }

    fn changes(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    fn take_control(&self, session: &str, tab: TabId) -> Result<TabHold, ErrorBody> {
        self.tab_in(session, Some(tab)).map(|(_, entry)| TabHold {
            session: session.to_string(),
            tab,
            hold: entry.control.take(),
        })
    }

    fn holds(&self, taken: &TabHold) -> bool {
        let sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        sessions
            .get(taken.session.as_str())
            .and_then(|entry| entry.tabs.get(&taken.tab))
            .is_some_and(|tab| taken.hold.holds(&tab.control))
    }

    fn type_as_human(&self, taken: &TabHold, input: &HumanInput) -> Result<(), ErrorBody> {
        let (session, tab, hold) = (taken.session.as_str(), taken.tab, &taken.hold);
        let time = SystemTime::now();
        let started = Instant::now();
        let (folder, terminal) = {
            let sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
            let found = sessions.get(session).and_then(|s| {
                let entry = s.tabs.get(&tab).filter(|entry| hold.holds(&entry.control))?;
                Some((Arc::clone(&s.folder), Arc::clone(&entry.terminal)))
            });
            found.ok_or_else(|| ErrorBody::failure(format!("{session} {tab} was closed")))?
        };
        let delivered = terminal.input(&input.input()).map(drop).map_err(|e| {
            let mut error = error_body(&e);
            error.message = format!("{session} {tab}: {}", error.message);
            error
        });
        let mut entry = ActivityEntry::at(&Typed { time, words: &input.call() }, started.elapsed());
        entry.tab = Some(tab.to_string());
        entry.frame = Some(terminal.frame());
        entry.human = Some(input.clone());
        if let Err(error) = &delivered {
            entry.exit = error.code;
            entry.error = Some(error.message.clone());
        }
        folder.log(&entry);
        delivered
    }
}

impl Session {
    fn new(folder: Arc<SessionFolder>) -> Self {
        Self { tabs: BTreeMap::new(), current: None, folder, created: Instant::now() }
    }

    fn info(&self, name: &Name) -> SessionInfo {
        SessionInfo {
            session: name.clone(),
            current: self.current.map(|id| id.to_string()),
            tabs: self.tabs.iter().map(|(id, tab)| tab.terminal.info(&id.to_string(), tab.control.owner())).collect(),
        }
    }
}

#[cfg(test)]
mod tests;
