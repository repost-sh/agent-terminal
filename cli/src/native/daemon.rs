use std::collections::BTreeMap;
use std::io::{ErrorKind, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::connection::{READY, home, socket_path};
use crate::native::pty;
use crate::native::terminal::{Settled, Terminal};
use crate::protocol::{self, Closed, ErrorBody, Reply, Request, Response, TabRef, Wait};

#[derive(Default)]
struct Daemon {
    sessions: Mutex<BTreeMap<String, Session>>,
}

#[derive(Default)]
struct Session {
    tabs: BTreeMap<u32, Arc<Terminal>>,
    current: Option<u32>,
    next_id: u32,
}

pub fn run() -> Result<()> {
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
        let request: Request = match protocol::read(&mut stream) {
            Ok(Some(request)) => request,
            Ok(None) => return,
            Err(e) => {
                eprintln!("reading a request failed: {e:#}");
                return;
            }
        };
        let stop = matches!(request, Request::Stop);
        let response = match self.handle(request) {
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
            Request::Snapshot { session } => {
                let (tab, terminal) = self.current(&session)?;
                Ok(Reply::Snapshot(terminal.snapshot(&session, &tab_name(tab))))
            }
            Request::Close { session, all } => {
                let closed = if all { self.close_all() } else { vec![self.close_current(&session)?] };
                Ok(Reply::Closed(Closed { closed }))
            }
            Request::Stop => {
                self.close_all();
                if let Err(e) = socket_path().and_then(|path| Ok(std::fs::remove_file(path)?)) {
                    eprintln!("removing the socket failed: {e:#}");
                }
                Ok(Reply::Stopped)
            }
        }
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
            tab_name(id)
        };
        let settled = terminal.wait(wait, Duration::from_millis(timeout_ms));
        let snapshot = terminal.snapshot(&session, &tab);
        match settled {
            Settled::Met | Settled::Exited => Ok(Reply::Snapshot(snapshot)),
            Settled::TimedOut => Err(ErrorBody {
                code: crate::exit::TIMEOUT,
                message: format!(
                    "timed out after {timeout_ms} ms waiting for {}; {session} {tab} is still running (read it with `agent-terminal snapshot`, stop it with `agent-terminal close`)",
                    describe(wait)
                ),
                snapshot: Some(Box::new(snapshot)),
            }),
        }
    }

    fn close_current(&self, session: &str) -> Result<TabRef, ErrorBody> {
        let mut sessions = self.sessions.lock().unwrap();
        let entry = sessions.get_mut(session).ok_or_else(|| no_terminal(session))?;
        let (id, terminal) =
            entry.current.and_then(|id| entry.tabs.remove(&id).map(|t| (id, t))).ok_or_else(|| no_terminal(session))?;
        terminal.kill();
        entry.current = entry.tabs.keys().next_back().copied();
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

    fn current(&self, session: &str) -> Result<(u32, Arc<Terminal>), ErrorBody> {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .get(session)
            .and_then(|s| s.current.and_then(|id| s.tabs.get(&id).map(|t| (id, Arc::clone(t)))))
            .ok_or_else(|| no_terminal(session))
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

fn error_body(error: anyhow::Error) -> ErrorBody {
    let code = error.downcast_ref::<crate::Error>().map_or(crate::exit::ERROR, |e| e.code);
    ErrorBody { code, message: format!("{error:#}"), snapshot: None }
}
