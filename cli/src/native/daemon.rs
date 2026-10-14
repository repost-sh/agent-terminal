use std::collections::BTreeMap;
use std::io::{ErrorKind, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::connection::{READY, home, socket_path};
use crate::native::pty;
use crate::native::terminal::{Settled, Terminal};
use crate::native::wait::{Condition, Pattern};
use crate::protocol::{
    self, Checked, Closed, Delivered, ErrorBody, Expect, Input, Marked, Matches, Printed, Reply, Request, Response,
    RowRange, Snapshot, SnapshotOptions, Style, TabRef, TextMatch, Wait, Waited,
};

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
    close_inherited_files()?;
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

fn close_inherited_files() -> Result<()> {
    let mut inherited = Vec::new();
    for entry in std::fs::read_dir("/dev/fd").context("list the files the daemon inherited")? {
        let name = entry.context("list the files the daemon inherited")?.file_name();
        if let Some(fd) = name.to_str().and_then(|name| name.parse::<i32>().ok()).filter(|&fd| fd > 2) {
            inherited.push(fd);
        }
    }
    for fd in inherited {
        match nix::unistd::close(fd) {
            Ok(()) | Err(nix::errno::Errno::EBADF) => {}
            Err(e) => return Err(e).with_context(|| format!("close inherited file descriptor {fd}")),
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
                self.open(session, pty::Command { program, args, cwd, env, size }, wait.as_ref(), timeout_ms)
            }
            Request::Snapshot { session, options } => {
                let (tab, terminal) = self.current(&session)?;
                terminal.snapshot_with(&session, &tab_name(tab), &options).map(Reply::Snapshot).map_err(failure)
            }
            Request::Close { session, all } => {
                let closed = if all { self.close_all() } else { vec![self.close_current(&session)?] };
                Ok(Reply::Closed(Closed { closed }))
            }
            Request::Get { session, what } => {
                let (_, terminal) = self.current(&session)?;
                terminal.fact(what).map(Reply::Fact).map_err(failure)
            }
            Request::Find { session, text, regex, all } => {
                let (_, terminal) = self.current(&session)?;
                let matches = terminal.find(&text, regex, all).map_err(failure)?;
                Ok(Reply::Matches(Matches { matches }))
            }
            Request::Mark { session, name } => {
                let (_, terminal) = self.current(&session)?;
                let frame = terminal.mark(name.clone());
                Ok(Reply::Marked(Marked { mark: name, frame }))
            }
            Request::Output { session, since, raw } => {
                let (_, terminal) = self.current(&session)?;
                let output = terminal.printed(since.as_deref(), raw).map_err(failure)?;
                Ok(Reply::Printed(Printed { output }))
            }
            Request::DiffSnapshot { session, baseline } => {
                let (_, terminal) = self.current(&session)?;
                terminal.diff_snapshot(baseline).map(Reply::Diff).map_err(failure)
            }
            Request::Input { session, input, wait, snapshot, timeout_ms } => {
                self.input(session, &input, wait.as_ref(), snapshot, timeout_ms)
            }
            Request::Wait { session, wait: Wait::Elapsed { ms }, timeout_ms } => {
                self.wait(session, &Wait::Elapsed { ms }, timeout_ms.max(ms))
            }
            Request::Wait { session, wait, timeout_ms } => self.wait(session, &wait, timeout_ms),
            Request::Expect { session, expect } => self.expect(session, &expect),
            Request::Stop => {
                self.close_all();
                if let Err(e) = socket_path().and_then(|path| Ok(std::fs::remove_file(path)?)) {
                    eprintln!("removing the socket failed: {e:#}");
                }
                Ok(Reply::Stopped)
            }
        }
    }

    fn open(
        &self,
        session: String,
        command: pty::Command,
        wait: Option<&Wait>,
        timeout_ms: u64,
    ) -> Result<Reply, ErrorBody> {
        let condition = wait.map(condition).transpose()?;
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
        if let Some((wait, condition)) = wait.zip(condition) {
            settle(&terminal, &session, &tab, wait, condition, timeout_ms)?;
        }
        Ok(Reply::Snapshot(terminal.snapshot(&session, &tab)))
    }

    fn input(
        &self,
        session: String,
        input: &Input,
        wait: Option<&Wait>,
        snapshot: bool,
        timeout_ms: u64,
    ) -> Result<Reply, ErrorBody> {
        let (id, terminal) = self.current(&session)?;
        let tab = tab_name(id);
        let condition = wait.map(condition).transpose()?;
        let delivery = terminal.input(input).map_err(|e| error_body(e.context(format!("{session} {tab}"))))?;
        if let Some((wait, condition)) = wait.zip(condition) {
            settle(&terminal, &session, &tab, wait, condition, timeout_ms)?;
        }
        let snapshot = snapshot.then(|| Box::new(terminal.snapshot(&session, &tab)));
        Ok(Reply::Delivered(Delivered { session, tab, delivery, snapshot }))
    }

    fn wait(&self, session: String, wait: &Wait, timeout_ms: u64) -> Result<Reply, ErrorBody> {
        let (id, terminal) = self.current(&session)?;
        let tab = tab_name(id);
        let condition = condition(wait)?;
        let started = Instant::now();
        let frame = settle(&terminal, &session, &tab, wait, condition, timeout_ms)?;
        let waited_ms = started.elapsed().as_millis() as u64;
        let exit_code = match wait {
            Wait::Exit { .. } => terminal.exit_status().flatten(),
            _ => None,
        };
        Ok(Reply::Waited(Waited { session, tab, until: describe(wait), waited_ms, frame, exit_code }))
    }

    fn expect(&self, session: String, expect: &Expect) -> Result<Reply, ErrorBody> {
        let (id, terminal) = self.current(&session)?;
        let tab = tab_name(id);
        let place = format!("{session} {tab}");
        let excerpt = |rows: Option<RowRange>| {
            let options = SnapshotOptions { rows, ..SnapshotOptions::default() };
            terminal.snapshot_with(&session, &tab, &options).map_err(failure)
        };
        let fact = match expect {
            Expect::Text(text) => {
                if !Pattern::new(text).map_err(failure)?.found(&terminal.lines()) {
                    return Err(assertion(format!("{text} is not on screen in {place}"), excerpt(None)?));
                }
                format!("{text} is on screen")
            }
            Expect::Gone(text) => {
                let rows = Pattern::new(text).map_err(failure)?.rows_found(&terminal.lines());
                if let (Some(&first), Some(&last)) = (rows.first(), rows.last()) {
                    let which = if first == last { format!("row {first}") } else { format!("rows {first}-{last}") };
                    let shown = excerpt(Some(RowRange { first, last }))?;
                    return Err(assertion(format!("{text} is on screen in {place}, in {which}"), shown));
                }
                format!("{text} is not on screen")
            }
            Expect::ExitCode { code } => match terminal.exit_status() {
                Some(Some(exited)) if exited == *code => format!("{} exited with code {code}", terminal.program),
                Some(_) => return Err(exited_otherwise(*code, terminal.snapshot(&session, &tab))),
                None => {
                    let message = format!(
                        "{} in {place} is still running, so it has not exited with code {code}",
                        terminal.program
                    );
                    return Err(assertion(message, excerpt(None)?));
                }
            },
            Expect::Cell { row, col, style } => {
                let cell = terminal.cell(*row, *col).map_err(failure)?;
                let missing: Vec<&Style> = style.iter().filter(|style| !style.holds(&cell)).collect();
                if !missing.is_empty() {
                    let message = format!("cell {row},{col} in {place} is not {}: it is {cell}", listed(&missing));
                    return Err(assertion(message, excerpt(Some(RowRange { first: *row, last: *row }))?));
                }
                format!("cell {row},{col} is {}", listed(&style.iter().collect::<Vec<_>>()))
            }
        };
        Ok(Reply::Checked(Checked { session, tab, fact }))
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

fn condition(wait: &Wait) -> Result<Condition, ErrorBody> {
    Condition::new(wait).map_err(failure)
}

fn settle(
    terminal: &Terminal,
    session: &str,
    tab: &str,
    wait: &Wait,
    condition: Condition,
    timeout_ms: u64,
) -> Result<u64, ErrorBody> {
    match terminal.wait(condition, Duration::from_millis(timeout_ms)) {
        Settled::Met { frame } => match (wait, terminal.exit_status()) {
            (Wait::Exit { code: Some(expected) }, Some(code)) if code != Some(*expected) => {
                Err(exited_otherwise(*expected, terminal.snapshot(session, tab)))
            }
            _ => Ok(frame),
        },
        Settled::Exited => Err(exited_while_waiting(wait, terminal.snapshot(session, tab))),
        Settled::TimedOut => Err(timed_out(wait, timeout_ms, terminal.snapshot(session, tab))),
    }
}

fn assertion(message: String, snapshot: Snapshot) -> ErrorBody {
    ErrorBody { code: crate::exit::ASSERTION, message, snapshot: Some(Box::new(snapshot)) }
}

fn listed(styles: &[&Style]) -> String {
    styles.iter().map(|style| style.to_string()).collect::<Vec<_>>().join(", ")
}

fn exited_otherwise(expected: i32, snapshot: Snapshot) -> ErrorBody {
    let how = snapshot.exit_code.map_or_else(|| "without an exit code".to_string(), |code| format!("with code {code}"));
    let message = format!("{} in {} {} exited {how}, not {expected}", snapshot.program, snapshot.session, snapshot.tab);
    assertion(message, snapshot)
}

fn exited_while_waiting(wait: &Wait, snapshot: Snapshot) -> ErrorBody {
    let how = snapshot.exit_code.map_or_else(String::new, |code| format!(" with code {code}"));
    ErrorBody {
        code: crate::exit::EXITED,
        message: format!(
            "stopped waiting for {}: {} in {} {} has exited{how}",
            describe(wait),
            snapshot.program,
            snapshot.session,
            snapshot.tab
        ),
        snapshot: Some(Box::new(snapshot)),
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
        Wait::Idle { quiet_ms } => format!("the program to print nothing for {quiet_ms} ms"),
        Wait::Elapsed { ms } => format!("{ms} ms to pass"),
        Wait::Exit { code: None } => "the program to exit".to_string(),
        Wait::Exit { code: Some(code) } => format!("the program to exit with code {code}"),
        Wait::Cursor { row, col } => format!("the cursor to reach {row},{col}"),
        Wait::Frame { frame } => format!("frame {frame}"),
        Wait::Text(text) => describe_text(text, "to appear", "a match for"),
        Wait::Gone(text) => describe_text(text, "to disappear", "no match for"),
    }
}

fn describe_text(text: &TextMatch, plain: &str, pattern: &str) -> String {
    let what = if text.regex { format!("{pattern} /{}/", text.text) } else { format!("\"{}\" {plain}", text.text) };
    match text.rows {
        Some(RowRange { first, last }) => format!("{what} in rows {first}-{last}"),
        None => what,
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

fn failure(message: String) -> ErrorBody {
    ErrorBody { code: crate::exit::ERROR, message, snapshot: None }
}

fn error_body(error: anyhow::Error) -> ErrorBody {
    let code = error.downcast_ref::<crate::Error>().map_or(crate::exit::ERROR, |e| e.code);
    ErrorBody { code, message: format!("{error:#}"), snapshot: None }
}
