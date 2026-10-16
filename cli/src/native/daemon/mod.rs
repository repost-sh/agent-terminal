mod activity;
mod attach;
mod batch;
mod dashboard_server;
mod dispatch;
mod expect;
mod input;
mod lifecycle;
mod reading;
mod refusals;
mod serve;
mod tabs;
mod waiting;

use std::collections::{BTreeMap, HashMap};
use std::io::{self, ErrorKind, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Condvar, Mutex, PoisonError, Weak};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use tokio::sync::watch;

use crate::home::{home, socket_path, version_path};
use crate::native::changes::Changes;
use crate::native::control::{Control, Hold};
use crate::native::dashboard::{self, Backend};
use crate::native::log;
use crate::native::store::{ActivityEntry, SessionFolder};
use crate::native::terminal::Terminal;
use crate::protocol::{ErrorBody, HumanInput, Reply, Request, SessionInfo, Sessions, TabId, Target};
use lifecycle::Activity;
use refusals::{error_body, no_tab, no_terminal, stopping};

pub(crate) const READY: &str = "ready";

struct Daemon {
    sessions: Mutex<BTreeMap<String, Session>>,
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
}

#[derive(Clone)]
struct Tab {
    terminal: Arc<Terminal>,
    control: Arc<Control>,
}

pub(crate) fn run(idle_timeout: Duration) -> Result<()> {
    log::start()?;
    let home = home()?;
    std::fs::create_dir_all(&home).with_context(|| format!("create {}", home.display()))?;
    let Some(listener) = listen()? else {
        return Ok(());
    };
    record_version(&home)?;

    let mut stdout = io::stdout();
    writeln!(stdout, "{READY}").and_then(|()| stdout.flush()).context("tell the CLI the daemon is ready")?;

    let (ended, end) = mpsc::channel();
    let daemon = Arc::new_cyclic(|me| Daemon::new(Weak::clone(me), ended));
    if !idle_timeout.is_zero() {
        let watching = Arc::clone(&daemon);
        std::thread::spawn(move || watching.stop_when_idle(idle_timeout));
    }
    let accepting = Arc::clone(&daemon);
    std::thread::spawn(move || accepting.accept(&listener));
    end.recv().context("every thread of the daemon ended without stopping it")
}

fn listen() -> Result<Option<UnixListener>> {
    let path = socket_path()?;
    match UnixListener::bind(&path) {
        Ok(listener) => Ok(Some(listener)),
        Err(e) if e.kind() == ErrorKind::AddrInUse && UnixStream::connect(&path).is_ok() => Ok(None),
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            std::fs::remove_file(&path).with_context(|| format!("remove the stale socket {}", path.display()))?;
            let listener = UnixListener::bind(&path).with_context(|| format!("listen on {}", path.display()))?;
            Ok(Some(listener))
        }
        Err(e) => Err(e).with_context(|| format!("listen on {}", path.display())),
    }
}

fn record_version(home: &Path) -> Result<()> {
    let path = version_path()?;
    let written = home.join("daemon.version.new");
    std::fs::write(&written, crate::version::VERSION).with_context(|| format!("write {}", written.display()))?;
    std::fs::rename(&written, &path).with_context(|| format!("move {} to {}", written.display(), path.display()))
}

impl Daemon {
    fn new(me: Weak<Daemon>, ended: Sender<()>) -> Self {
        Self {
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
        let Target { session, tab } = target;
        let sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = sessions.get(session);
        match *tab {
            Some(id) => entry.and_then(|s| s.tabs.get(&id)).map(|t| (id, t.clone())).ok_or_else(|| no_tab(session, id)),
            None => entry
                .and_then(|s| s.current.and_then(|id| s.tabs.get(&id).map(|t| (id, t.clone()))))
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

    fn take_control(&self, session: &str, tab: TabId) -> Result<Hold, ErrorBody> {
        self.tab(&Target::tab(session, tab)).map(|(_, tab)| tab.control.take())
    }

    fn holds(&self, session: &str, tab: TabId, hold: &Hold) -> bool {
        let sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        sessions.get(session).and_then(|entry| entry.tabs.get(&tab)).is_some_and(|tab| hold.holds(&tab.control))
    }

    fn type_as_human(&self, session: &str, tab: TabId, hold: &Hold, input: &HumanInput) -> Result<(), ErrorBody> {
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
        let mut entry = ActivityEntry::at(time, &input.call(), started.elapsed());
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
        Self { tabs: BTreeMap::new(), current: None, folder }
    }

    fn info(&self, name: &str) -> SessionInfo {
        SessionInfo {
            session: name.to_string(),
            current: self.current.map(|id| id.to_string()),
            tabs: self.tabs.iter().map(|(id, tab)| tab.terminal.info(&id.to_string(), tab.control.owner())).collect(),
        }
    }
}
