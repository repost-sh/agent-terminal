use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::connection::{home, socket_path};
use crate::native::pty;
use crate::native::screen::Size;
use crate::native::terminal::{Settled, Terminal};
use crate::protocol::{self, Closed, ErrorBody, Request, Response, Snapshot, TabRef};

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
    let home = home();
    std::fs::create_dir_all(&home).with_context(|| format!("create {}", home.display()))?;
    let path = socket_path();
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            if UnixStream::connect(&path).is_ok() {
                return Ok(());
            }
            std::fs::remove_file(&path)?;
            UnixListener::bind(&path)?
        }
        Err(e) => return Err(e).with_context(|| format!("listen on {}", path.display())),
    };

    let daemon = Arc::new(Daemon::default());
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let daemon = Arc::clone(&daemon);
        std::thread::spawn(move || daemon.serve(stream));
    }
    Ok(())
}

impl Daemon {
    fn serve(&self, mut stream: UnixStream) {
        let request: Request = match protocol::read(&mut stream) {
            Ok(request) => request,
            Err(_) => return,
        };
        let stop = matches!(request, Request::Stop);
        let response = match self.handle(request) {
            Ok(value) => Response::Ok { ok: value },
            Err(failure) => Response::Err { error: failure },
        };
        let _ = protocol::write(&mut stream, &response);
        if stop {
            let _ = std::fs::remove_file(socket_path());
            std::process::exit(0);
        }
    }

    fn handle(&self, request: Request) -> Result<serde_json::Value, ErrorBody> {
        match request {
            Request::Open { session, program, args, cols, rows, cwd, env, wait_stable_ms, timeout_ms } => {
                let terminal = Terminal::start(pty::Command { program, args, cwd, env, size: Size { cols, rows } })
                    .map_err(error_body)?;
                let tab = {
                    let mut sessions = self.sessions.lock().unwrap();
                    let session = sessions.entry(session.clone()).or_default();
                    session.next_id += 1;
                    let id = session.next_id;
                    session.tabs.insert(id, Arc::clone(&terminal));
                    session.current = Some(id);
                    id
                };
                let settled =
                    terminal.wait_stable(Duration::from_millis(wait_stable_ms), Duration::from_millis(timeout_ms));
                let snapshot = snapshot_of(&session, tab, &terminal);
                if let Settled::TimedOut = settled {
                    return Err(ErrorBody {
                        code: crate::EXIT_TIMEOUT,
                        message: format!("timed out after {timeout_ms} ms waiting for the screen to settle"),
                        snapshot: Some(Box::new(snapshot)),
                    });
                }
                Ok(to_value(snapshot))
            }
            Request::Snapshot { session } => {
                let (tab, terminal) = self.current(&session)?;
                Ok(to_value(snapshot_of(&session, tab, &terminal)))
            }
            Request::Close { session, all } => {
                let mut closed = Vec::new();
                let mut sessions = self.sessions.lock().unwrap();
                if all {
                    for (name, session) in std::mem::take(&mut *sessions) {
                        for (id, terminal) in session.tabs {
                            terminal.kill();
                            closed.push(TabRef { session: name.clone(), tab: tab_name(id) });
                        }
                    }
                } else {
                    let Some(s) = sessions.get_mut(&session) else { return Err(no_terminal(&session)) };
                    let Some((id, terminal)) = s.current.and_then(|id| s.tabs.remove(&id).map(|t| (id, t))) else {
                        return Err(no_terminal(&session));
                    };
                    terminal.kill();
                    s.current = s.tabs.keys().next_back().copied();
                    if s.tabs.is_empty() {
                        sessions.remove(&session);
                    }
                    closed.push(TabRef { session, tab: tab_name(id) });
                }
                Ok(to_value(Closed { closed }))
            }
            Request::Stop => {
                for session in std::mem::take(&mut *self.sessions.lock().unwrap()).into_values() {
                    for terminal in session.tabs.into_values() {
                        terminal.kill();
                    }
                }
                Ok(serde_json::json!({ "stopped": true }))
            }
        }
    }

    fn current(&self, session: &str) -> Result<(u32, Arc<Terminal>), ErrorBody> {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .get(session)
            .and_then(|s| s.current.and_then(|id| s.tabs.get(&id).map(|t| (id, Arc::clone(t)))))
            .ok_or_else(|| no_terminal(session))
    }
}

fn snapshot_of(session: &str, tab: u32, terminal: &Terminal) -> Snapshot {
    let capture = terminal.capture();
    let view = capture.view;
    Snapshot {
        session: session.to_string(),
        tab: tab_name(tab),
        program: terminal.program.clone(),
        cols: view.size.cols,
        rows: view.size.rows,
        alt_screen: view.alt_screen,
        mouse: view.mouse.map(|m| m.as_str().to_string()),
        cursor: [view.cursor.0 + 1, view.cursor.1 + 1],
        cursor_visible: view.cursor_visible,
        frame: capture.frame,
        exit_code: capture.exit_code,
        lines: view.lines,
    }
}

fn tab_name(id: u32) -> String {
    format!("t{id}")
}

fn no_terminal(session: &str) -> ErrorBody {
    ErrorBody {
        code: 1,
        message: format!("no terminal is open in session {session}; start one with `agent-terminal open <program>`"),
        snapshot: None,
    }
}

fn error_body(error: anyhow::Error) -> ErrorBody {
    let code = error.downcast_ref::<crate::Error>().map_or(1, |e| e.code);
    ErrorBody { code, message: format!("{error:#}"), snapshot: None }
}

fn to_value<T: serde::Serialize>(value: T) -> serde_json::Value {
    serde_json::to_value(value).expect("protocol types serialize")
}
