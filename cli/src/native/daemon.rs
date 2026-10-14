use std::collections::{BTreeMap, HashMap};
use std::io::{ErrorKind, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};

use crate::connection::{READY, home, socket_path};
use crate::native::pty;
use crate::native::screenshots;
use crate::native::store::{self, ActivityEntry, SessionFolder};
use crate::native::terminal::{Settled, Terminal};
use crate::native::wait::{Condition, Pattern};
use crate::protocol::{
    self, Addressed, BatchOutcome, BatchStep, Call, Checked, Closed, Delivered, ErrorBody, Expect, Export, Input,
    Marked, Matches, Printed, RecordingFile, Reply, Request, Response, RowRange, SavedRecording, SessionInfo, Sessions,
    Shot, Size, Snapshot, SnapshotOptions, Style, TabRef, TextMatch, Wait, Waited,
};

#[derive(Default)]
struct Daemon {
    sessions: Mutex<BTreeMap<String, Session>>,
    restarting: Mutex<()>,
    activity: Mutex<Activity>,
    quiet: Condvar,
    folders: Mutex<HashMap<String, Weak<SessionFolder>>>,
}

struct Session {
    tabs: BTreeMap<u32, Arc<Terminal>>,
    current: Option<u32>,
    folder: Arc<SessionFolder>,
}

struct Logged {
    session: String,
    tab: Option<u32>,
    folder: Option<Arc<SessionFolder>>,
    typed: Vec<String>,
    time: SystemTime,
    started: Instant,
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
        let call: Call = match protocol::read(&mut stream) {
            Ok(Some(call)) => call,
            Ok(None) => return,
            Err(e) => {
                eprintln!("reading a request failed: {e:#}");
                return;
            }
        };
        let stop = matches!(call.request, Request::Stop);
        let logged = call.typed.and_then(|typed| self.about_to_handle(typed, &call.request));
        let handled = match &busy {
            Some(_) => self.handle(call.request),
            None => Err(ErrorBody {
                code: crate::exit::ERROR,
                message: "the daemon is stopping because it was idle; run the command again to start a new one".into(),
                snapshot: None,
            }),
        };
        if let Some(logged) = logged {
            self.log(logged, &handled);
        }
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
                self.open(session, pty::Command { program, args, cwd, env, size }, wait.as_ref(), timeout_ms)
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
            Request::Input { session, tab, input, wait, snapshot, screenshot, timeout_ms } => {
                let after = After { wait: wait.as_ref(), snapshot, screenshot: screenshot.as_ref() };
                self.input(session, tab, &input, after, timeout_ms)
            }
            Request::Screenshot { session, tab, shot } => {
                let (id, terminal) = self.target(&session, tab)?;
                screenshots::take(&session, &tab_name(id), &terminal, &shot).map(Reply::Screenshot).map_err(error_body)
            }
            Request::DiffScreenshot { session, tab, baseline, shot, threshold } => {
                let (id, terminal) = self.target(&session, tab)?;
                screenshots::diff(&session, &tab_name(id), &terminal, baseline, &shot, threshold)
                    .map(Reply::ScreenshotDiff)
                    .map_err(error_body)
            }
            Request::Wait { session, tab, wait: Wait::Elapsed { ms }, timeout_ms } => {
                self.wait(session, tab, &Wait::Elapsed { ms }, timeout_ms.max(ms))
            }
            Request::Wait { session, tab, wait, timeout_ms } => self.wait(session, tab, &wait, timeout_ms),
            Request::Expect { session, tab, expect } => self.expect(session, tab, &expect),
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
            Request::RecordStart { session, tab, path } => {
                let (id, terminal) = self.target(&session, tab)?;
                let tab = tab_name(id);
                let path = match path {
                    Some(path) => path,
                    None => home().map_err(error_body)?.join("recordings").join(format!(
                        "{}-{tab}-{}.cast",
                        store::file_name(&session),
                        terminal.frame()
                    )),
                };
                terminal.recording().save_to(&path).map_err(error_body)?;
                Ok(Reply::Recording(RecordingFile { session, tab, path }))
            }
            Request::RecordStop { session, tab } => {
                let (id, terminal) = self.target(&session, tab)?;
                let saved = terminal.recording().stop_saving().map_err(error_body)?;
                Ok(Reply::Saved(SavedRecording {
                    session,
                    tab: tab_name(id),
                    path: saved.path,
                    duration_ms: saved.duration.as_millis() as u64,
                    bytes: saved.bytes,
                }))
            }
            Request::TraceExport { session, path } => {
                let exported = home().and_then(|home| store::export(&home, &session, &path)).map_err(error_body)?;
                Ok(Reply::Exported(Export { session, path, files: exported.files, bytes: exported.bytes }))
            }
            Request::Stop => {
                self.shut_down();
                Ok(Reply::Stopped)
            }
            Request::Batch { steps, bail } => Ok(Reply::Batch(self.batch(steps, bail))),
        }
    }

    fn batch(&self, steps: Vec<BatchStep>, bail: bool) -> Vec<BatchOutcome> {
        let mut outcomes = Vec::with_capacity(steps.len());
        let mut failed = false;
        for step in steps {
            let response = if bail && failed { None } else { Some(self.batch_step(&step.args, step.request)) };
            failed |= response.as_ref().is_some_and(|response| response.exit_code() != 0);
            outcomes.push(BatchOutcome { args: step.args, response });
        }
        outcomes
    }

    fn batch_step(&self, args: &[String], request: Request) -> Response {
        if matches!(request, Request::Stop) {
            return Response::Err(ErrorBody {
                code: crate::exit::ERROR,
                message: "stopping the daemon cannot be part of a batch".into(),
                snapshot: None,
            });
        }
        let logged = self.about_to_handle(args.to_vec(), &request);
        let handled = self.handle(request);
        if let Some(logged) = logged {
            self.log(logged, &handled);
        }
        match handled {
            Ok(reply) => Response::Ok(reply),
            Err(failure) => Response::Err(failure),
        }
    }

    fn about_to_handle(&self, typed: Vec<String>, request: &Request) -> Option<Logged> {
        let sessions = self.sessions.lock().unwrap();
        let (session, tab) = match request.addressed() {
            Addressed::Nothing => return None,
            Addressed::Session(session) => (session, None),
            Addressed::Tab(session, Some(id)) => (session, Some(id)),
            Addressed::Tab(session, None) => (session, sessions.get(session).and_then(|s| s.current)),
        };
        Some(Logged {
            session: session.to_string(),
            tab,
            folder: sessions.get(session).map(|s| Arc::clone(&s.folder)),
            typed,
            time: SystemTime::now(),
            started: Instant::now(),
        })
    }

    fn log(&self, logged: Logged, handled: &Result<Reply, ErrorBody>) {
        let mut entry = ActivityEntry::at(logged.time, &logged.typed, logged.started.elapsed());
        let folder =
            logged.folder.or_else(|| self.sessions.lock().unwrap().get(&logged.session).map(|s| Arc::clone(&s.folder)));
        let Some(folder) = folder else {
            return;
        };
        let shown = match handled {
            Ok(Reply::Snapshot(snapshot)) => Some(snapshot),
            Err(failure) => failure.snapshot.as_deref(),
            Ok(_) => None,
        };
        entry.tab = shown.map(|snapshot| snapshot.tab.clone()).or_else(|| logged.tab.map(tab_name));
        entry.frame = shown.map(|snapshot| snapshot.frame).or_else(|| {
            let (_, terminal) = self.target(&logged.session, Some(logged.tab?)).ok()?;
            Some(terminal.frame())
        });
        match handled {
            Ok(reply) => entry.exit = reply.exit_code(),
            Err(failure) => {
                entry.exit = failure.code;
                entry.error = Some(failure.message.clone());
            }
        }
        if let Ok(Reply::ScreenshotDiff(diff)) = handled
            && !diff.passed
        {
            entry.error = Some(diff.summary());
        }
        entry.screenshots = match handled {
            Ok(Reply::Screenshot(screenshot)) => vec![screenshot.path.clone()],
            Ok(Reply::ScreenshotDiff(diff)) => vec![diff.path.clone()],
            Ok(Reply::Delivered(delivered)) => delivered.screenshot.iter().map(|shot| shot.path.clone()).collect(),
            Ok(_) | Err(_) => Vec::new(),
        };
        folder.log(&entry);
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

    fn open(
        &self,
        session: String,
        command: pty::Command,
        wait: Option<&Wait>,
        timeout_ms: u64,
    ) -> Result<Reply, ErrorBody> {
        let spawned = pty::spawn(command.clone()).map_err(error_body)?;
        let folder = match self.folder(&session) {
            Ok(folder) => folder,
            Err(e) => {
                spawned.abandon();
                return Err(error_body(e));
            }
        };
        let tab = folder.next_tab();
        let recording = folder.record(&tab_name(tab), &command);
        let terminal = Terminal::start(command, spawned, recording);
        {
            let mut sessions = self.sessions.lock().unwrap();
            let entry = sessions.entry(session.clone()).or_insert_with(|| Session::new(Arc::clone(&folder)));
            entry.tabs.insert(tab, Arc::clone(&terminal));
            entry.current = Some(tab);
        }
        match wait {
            Some(wait) => settle(&session, tab, &terminal, wait, timeout_ms),
            None => Ok(Reply::Snapshot(terminal.snapshot(&session, &tab_name(tab)))),
        }
    }

    fn folder(&self, session: &str) -> Result<Arc<SessionFolder>> {
        let mut folders = self.folders.lock().unwrap();
        if let Some(folder) = folders.get(session).and_then(Weak::upgrade) {
            return Ok(folder);
        }
        let folder = Arc::new(SessionFolder::start(&home()?, session)?);
        folders.retain(|_, folder| folder.strong_count() > 0);
        folders.insert(session.to_string(), Arc::downgrade(&folder));
        Ok(folder)
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
        let command = old.rerun();
        let spawned = pty::spawn(command.clone()).map_err(error_body)?;
        let recording = old.recording();
        recording.restart();
        let terminal = Terminal::start(command, spawned, recording);
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
        after: After<'_>,
        timeout_ms: u64,
    ) -> Result<Reply, ErrorBody> {
        let (id, terminal) = self.target(&session, tab)?;
        let tab = tab_name(id);
        let condition = after.wait.map(condition).transpose()?;
        let delivery = terminal.input(input).map_err(|e| error_body(e.context(format!("{session} {tab}"))))?;
        if let Some((wait, condition)) = after.wait.zip(condition) {
            wait_for(&terminal, &session, &tab, wait, condition, timeout_ms)?;
        }
        let screenshot = after
            .screenshot
            .map(|shot| screenshots::take(&session, &tab, &terminal, shot).map_err(error_body))
            .transpose()?
            .map(Box::new);
        let snapshot = after.snapshot.then(|| Box::new(terminal.snapshot(&session, &tab)));
        Ok(Reply::Delivered(Delivered { session, tab, delivery, screenshot, snapshot }))
    }

    fn wait(&self, session: String, tab: Option<u32>, wait: &Wait, timeout_ms: u64) -> Result<Reply, ErrorBody> {
        let (id, terminal) = self.target(&session, tab)?;
        let tab = tab_name(id);
        let condition = condition(wait)?;
        let started = Instant::now();
        let frame = wait_for(&terminal, &session, &tab, wait, condition, timeout_ms)?;
        let waited_ms = started.elapsed().as_millis() as u64;
        let exit_code = match wait {
            Wait::Exit { .. } => terminal.exit_status().flatten(),
            _ => None,
        };
        Ok(Reply::Waited(Waited { session, tab, until: describe(wait), waited_ms, frame, exit_code }))
    }

    fn expect(&self, session: String, tab: Option<u32>, expect: &Expect) -> Result<Reply, ErrorBody> {
        let (id, terminal) = self.target(&session, tab)?;
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
    fn new(folder: Arc<SessionFolder>) -> Self {
        Self { tabs: BTreeMap::new(), current: None, folder }
    }

    fn info(&self, name: &str) -> SessionInfo {
        SessionInfo {
            session: name.to_string(),
            current: self.current.map(tab_name),
            tabs: self.tabs.iter().map(|(id, terminal)| terminal.info(&tab_name(*id))).collect(),
        }
    }
}

struct After<'a> {
    wait: Option<&'a Wait>,
    snapshot: bool,
    screenshot: Option<&'a Shot>,
}

fn settle(session: &str, tab: u32, terminal: &Terminal, wait: &Wait, timeout_ms: u64) -> Result<Reply, ErrorBody> {
    let tab = tab_name(tab);
    let settled = terminal.wait(condition(wait)?, Duration::from_millis(timeout_ms));
    let snapshot = terminal.snapshot(session, &tab);
    match settled {
        Settled::Met { .. } | Settled::Exited => Ok(Reply::Snapshot(snapshot)),
        Settled::TimedOut => Err(timed_out(wait, timeout_ms, snapshot)),
    }
}

fn condition(wait: &Wait) -> Result<Condition, ErrorBody> {
    Condition::new(wait).map_err(failure)
}

fn wait_for(
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
