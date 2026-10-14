use std::collections::BTreeMap;
use std::io::{ErrorKind, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::connection::{READY, home, socket_path};
use crate::native::pty;
use crate::native::terminal::{Settled, Terminal};
use crate::protocol::{
    self, Closed, Delivered, ErrorBody, Input, Marked, Matches, Printed, Reply, Request, Response, SessionInfo,
    Sessions, Size, Snapshot, TabRef, Wait,
};

#[derive(Default)]
struct Daemon {
    sessions: Mutex<BTreeMap<String, Session>>,
    restarting: Mutex<()>,
    activity: Mutex<Activity>,
    quiet: Condvar,
}

#[derive(Default)]
struct Session {
    tabs: BTreeMap<u32, Arc<Terminal>>,
    current: Option<u32>,
    next_id: u32,
}

struct Activity {
    in_flight: usize,
    last: Instant,
    stopping: bool,
}

impl Default for Activity {
    fn default() -> Self {
        Self { in_flight: 0, last: Instant::now(), stopping: false }
    }
}

struct Busy<'a>(&'a Daemon);

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        let mut activity = self.0.activity.lock().unwrap();
        activity.in_flight -= 1;
        activity.last = Instant::now();
        self.0.quiet.notify_all();
    }
}

pub fn run(idle_timeout: Duration) -> Result<()> {
    let home = home()?;
    std::fs::create_dir_all(&home).with_context(|| format!("create {}", home.display()))?;
    let path = socket_path()?;
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(e) if e.kind() == ErrorKind::AddrInUse && UnixStream::connect(&path).is_ok() => return Ok(()),
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            std::fs::remove_file(&path).with_context(|| format!("remove the stale socket {}", path.display()))?;
            UnixListener::bind(&path).with_context(|| format!("listen on {}", path.display()))?
        }
        Err(e) => return Err(e).with_context(|| format!("listen on {}", path.display())),
    };

    let mut stdout = std::io::stdout();
    writeln!(stdout, "{READY}").and_then(|()| stdout.flush()).context("tell the CLI the daemon is ready")?;

    let daemon = Arc::new(Daemon::default());
    if !idle_timeout.is_zero() {
        let watching = Arc::clone(&daemon);
        std::thread::spawn(move || watching.stop_when_idle(idle_timeout));
    }
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let daemon = Arc::clone(&daemon);
                std::thread::spawn(move || daemon.serve(stream));
            }
            Err(e) => eprintln!("accepting a connection failed: {e}"),
        }
    }
    Ok(())
}

impl Daemon {
    fn serve(&self, mut stream: UnixStream) {
        let busy = self.busy();
        let request: Request = match protocol::read(&mut stream) {
            Ok(Some(request)) => request,
            Ok(None) => return,
            Err(e) => {
                eprintln!("reading a request failed: {e:#}");
                return;
            }
        };
        let stop = matches!(request, Request::Stop);
        let handled = match &busy {
            Some(_) => self.handle(request),
            None => Err(ErrorBody {
                code: crate::exit::ERROR,
                message: "the daemon is stopping because it was idle; run the command again to start a new one".into(),
                snapshot: None,
            }),
        };
        let response = match handled {
            Ok(reply) => Response::Ok(reply),
            Err(failure) => Response::Err(failure),
        };
        if let Err(e) = protocol::write(&mut stream, &response) {
            eprintln!("sending a reply failed: {e:#}");
        }
        if stop {
            std::process::exit(0);
        }
    }

    fn handle(&self, request: Request) -> Result<Reply, ErrorBody> {
        match request {
            Request::Open { session, program, args, size, cwd, env, wait, timeout_ms } => {
                self.open(session, pty::Command { program, args, cwd, env, size }, &wait, timeout_ms)
            }
            Request::Snapshot { session, tab, options } => {
                let (tab, terminal) = self.target(&session, tab)?;
                terminal.snapshot_with(&session, &tab_name(tab), &options).map(Reply::Snapshot).map_err(failure)
            }
            Request::Close { session, all, tab } => {
                let closed = if all { self.close_all() } else { vec![self.close(&session, tab)?] };
                Ok(Reply::Closed(Closed { closed }))
            }
            Request::Resize { session, tab, size, wait, timeout_ms } => {
                self.resize(&session, tab, size, &wait, timeout_ms)
            }
            Request::Restart { session, tab, wait, timeout_ms } => self.restart(&session, tab, &wait, timeout_ms),
            Request::Sessions => {
                let sessions = self.sessions.lock().unwrap();
                Ok(Reply::Sessions(Sessions {
                    sessions: sessions.iter().map(|(name, session)| session.info(name)).collect(),
                }))
            }
            Request::Tabs { session } => {
                let sessions = self.sessions.lock().unwrap();
                Ok(Reply::Tabs(
                    sessions.get(&session).map_or_else(|| SessionInfo::empty(&session), |s| s.info(&session)),
                ))
            }
            Request::SwitchTab { session, tab } => {
                let mut sessions = self.sessions.lock().unwrap();
                let entry = sessions
                    .get_mut(&session)
                    .filter(|s| s.tabs.contains_key(&tab))
                    .ok_or_else(|| no_tab(&session, tab))?;
                entry.current = Some(tab);
                Ok(Reply::Tabs(entry.info(&session)))
            }
            Request::Input { session, tab, input, wait, timeout_ms } => {
                self.input(session, tab, &input, wait.as_ref(), timeout_ms)
            }
            Request::Get { session, tab, what } => {
                let (_, terminal) = self.target(&session, tab)?;
                terminal.fact(what).map(Reply::Fact).map_err(failure)
            }
            Request::Find { session, tab, text, regex, all } => {
                let (_, terminal) = self.target(&session, tab)?;
                let matches = terminal.find(&text, regex, all).map_err(failure)?;
                Ok(Reply::Matches(Matches { matches }))
            }
            Request::Mark { session, tab, name } => {
                let (_, terminal) = self.target(&session, tab)?;
                let frame = terminal.mark(name.clone());
                Ok(Reply::Marked(Marked { mark: name, frame }))
            }
            Request::Output { session, tab, since, raw } => {
                let (_, terminal) = self.target(&session, tab)?;
                let output = terminal.printed(since.as_deref(), raw).map_err(failure)?;
                Ok(Reply::Printed(Printed { output }))
            }
            Request::DiffSnapshot { session, tab, baseline } => {
                let (_, terminal) = self.target(&session, tab)?;
                terminal.diff_snapshot(baseline).map(Reply::Diff).map_err(failure)
            }
            Request::Stop => {
                self.shut_down();
                Ok(Reply::Stopped)
            }
        }
    }

    fn busy(&self) -> Option<Busy<'_>> {
        let mut activity = self.activity.lock().unwrap();
        if activity.stopping {
            return None;
        }
        activity.in_flight += 1;
        Some(Busy(self))
    }

    fn stop_when_idle(&self, timeout: Duration) {
        let mut activity = self.activity.lock().unwrap();
        loop {
            let quiet_for = activity.last.elapsed();
            if activity.in_flight == 0 && quiet_for >= timeout {
                break;
            }
            activity = if activity.in_flight > 0 {
                self.quiet.wait(activity).unwrap()
            } else {
                self.quiet.wait_timeout(activity, timeout - quiet_for).unwrap().0
            };
        }
        activity.stopping = true;
        drop(activity);
        eprintln!("no call for {} ms, stopping", timeout.as_millis());
        self.shut_down();
        std::process::exit(0);
    }

    fn shut_down(&self) {
        if let Err(e) = socket_path().and_then(|path| Ok(std::fs::remove_file(path)?)) {
            eprintln!("removing the socket failed: {e:#}");
        }
        self.close_all();
    }

    fn open(&self, session: String, command: pty::Command, wait: &Wait, timeout_ms: u64) -> Result<Reply, ErrorBody> {
        let terminal = Terminal::start(command).map_err(error_body)?;
        let tab = {
            let mut sessions = self.sessions.lock().unwrap();
            let entry = sessions.entry(session.clone()).or_default();
            entry.next_id += 1;
            let id = entry.next_id;
            entry.tabs.insert(id, Arc::clone(&terminal));
            entry.current = Some(id);
            id
        };
        settle(&session, tab, &terminal, wait, timeout_ms)
    }

    fn resize(
        &self,
        session: &str,
        tab: Option<u32>,
        size: Size,
        wait: &Wait,
        timeout_ms: u64,
    ) -> Result<Reply, ErrorBody> {
        let (tab, terminal) = self.target(session, tab)?;
        terminal.resize(size).map_err(|e| error_body(e.context(format!("{session} {}", tab_name(tab)))))?;
        settle(session, tab, &terminal, wait, timeout_ms)
    }

    fn restart(&self, session: &str, tab: Option<u32>, wait: &Wait, timeout_ms: u64) -> Result<Reply, ErrorBody> {
        let restarting = self.restarting.lock().unwrap();
        let (tab, old) = self.target(session, tab)?;
        if !old.stop(Duration::from_millis(timeout_ms)) {
            return Err(ErrorBody {
                code: crate::exit::TIMEOUT,
                message: format!(
                    "{} in {session} {} did not stop within {timeout_ms} ms, so it was not restarted",
                    old.program,
                    tab_name(tab)
                ),
                snapshot: None,
            });
        }
        let terminal = Terminal::start(old.rerun()).map_err(error_body)?;
        {
            let mut sessions = self.sessions.lock().unwrap();
            match sessions.get_mut(session).and_then(|s| s.tabs.get_mut(&tab)) {
                Some(slot) if Arc::ptr_eq(slot, &old) => *slot = Arc::clone(&terminal),
                _ => {
                    terminal.kill();
                    return Err(no_tab(session, tab));
                }
            }
        }
        drop(restarting);
        settle(session, tab, &terminal, wait, timeout_ms)
    }

    fn input(
        &self,
        session: String,
        tab: Option<u32>,
        input: &Input,
        wait: Option<&Wait>,
        timeout_ms: u64,
    ) -> Result<Reply, ErrorBody> {
        let (id, terminal) = self.target(&session, tab)?;
        let tab = tab_name(id);
        let delivery = terminal.input(input).map_err(|e| error_body(e.context(format!("{session} {tab}"))))?;
        if let Some(wait) = wait
            && let Settled::TimedOut = terminal.wait(wait, Duration::from_millis(timeout_ms))
        {
            return Err(timed_out(wait, timeout_ms, terminal.snapshot(&session, &tab)));
        }
        Ok(Reply::Delivered(Delivered { session, tab, delivery }))
    }

    fn close(&self, session: &str, tab: Option<u32>) -> Result<TabRef, ErrorBody> {
        let mut sessions = self.sessions.lock().unwrap();
        let missing = || tab.map_or_else(|| no_terminal(session), |id| no_tab(session, id));
        let entry = sessions.get_mut(session).ok_or_else(missing)?;
        let id = tab.or(entry.current).ok_or_else(missing)?;
        let terminal = entry.tabs.remove(&id).ok_or_else(missing)?;
        terminal.kill();
        if entry.current == Some(id) {
            entry.current = entry.tabs.keys().next_back().copied();
        }
        if entry.tabs.is_empty() {
            sessions.remove(session);
        }
        Ok(TabRef { session: session.to_string(), tab: tab_name(id) })
    }

    fn close_all(&self) -> Vec<TabRef> {
        let sessions = std::mem::take(&mut *self.sessions.lock().unwrap());
        let mut closed = Vec::new();
        for (name, session) in sessions {
            for (id, terminal) in session.tabs {
                terminal.kill();
                closed.push(TabRef { session: name.clone(), tab: tab_name(id) });
            }
        }
        closed
    }

    fn target(&self, session: &str, tab: Option<u32>) -> Result<(u32, Arc<Terminal>), ErrorBody> {
        let sessions = self.sessions.lock().unwrap();
        let entry = sessions.get(session);
        match tab {
            Some(id) => {
                entry.and_then(|s| s.tabs.get(&id)).map(|t| (id, Arc::clone(t))).ok_or_else(|| no_tab(session, id))
            }
            None => entry
                .and_then(|s| s.current.and_then(|id| s.tabs.get(&id).map(|t| (id, Arc::clone(t)))))
                .ok_or_else(|| no_terminal(session)),
        }
    }
}

impl Session {
    fn info(&self, name: &str) -> SessionInfo {
        SessionInfo {
            session: name.to_string(),
            current: self.current.map(tab_name),
            tabs: self.tabs.iter().map(|(id, terminal)| terminal.info(&tab_name(*id))).collect(),
        }
    }
}

fn settle(session: &str, tab: u32, terminal: &Terminal, wait: &Wait, timeout_ms: u64) -> Result<Reply, ErrorBody> {
    let tab = tab_name(tab);
    let settled = terminal.wait(wait, Duration::from_millis(timeout_ms));
    let snapshot = terminal.snapshot(session, &tab);
    match settled {
        Settled::Met | Settled::Exited => Ok(Reply::Snapshot(snapshot)),
        Settled::TimedOut => Err(timed_out(wait, timeout_ms, snapshot)),
    }
}

fn timed_out(wait: &Wait, timeout_ms: u64, snapshot: Snapshot) -> ErrorBody {
    ErrorBody {
        code: crate::exit::TIMEOUT,
        message: format!(
            "timed out after {timeout_ms} ms waiting for {}; {} {} is still running (read it with `agent-terminal snapshot`, stop it with `agent-terminal close`)",
            describe(wait),
            snapshot.session,
            snapshot.tab
        ),
        snapshot: Some(Box::new(snapshot)),
    }
}

fn describe(wait: &Wait) -> String {
    match wait {
        Wait::Stable { quiet_ms } => format!("the screen to stay unchanged for {quiet_ms} ms"),
        Wait::Text { text } => format!("\"{text}\" to appear"),
    }
}

fn tab_name(id: u32) -> String {
    format!("t{id}")
}

fn no_terminal(session: &str) -> ErrorBody {
    ErrorBody {
        code: crate::exit::ERROR,
        message: format!("no terminal is open in session {session}; start one with `agent-terminal open <program>`"),
        snapshot: None,
    }
}

fn no_tab(session: &str, id: u32) -> ErrorBody {
    failure(format!("no tab {} in session {session}; list tabs with `agent-terminal tab`", tab_name(id)))
}

fn failure(message: String) -> ErrorBody {
    ErrorBody { code: crate::exit::ERROR, message, snapshot: None }
}

fn error_body(error: anyhow::Error) -> ErrorBody {
    let code = error.downcast_ref::<crate::Error>().map_or(crate::exit::ERROR, |e| e.code);
    ErrorBody { code, message: format!("{error:#}"), snapshot: None }
}
