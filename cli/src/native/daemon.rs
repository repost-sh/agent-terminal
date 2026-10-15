use std::collections::{BTreeMap, HashMap};
use std::io::{ErrorKind, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use tokio::sync::watch;

use crate::connection::{READY, home, socket_path};
use crate::native::changes::Changes;
use crate::native::clips;
use crate::native::control::{Control, Handed, Hold};
use crate::native::dashboard::{self, Backend};
use crate::native::pty;
use crate::native::screenshots;
use crate::native::shell;
use crate::native::store::{self, ActivityEntry, Pending, SessionFolder};
use crate::native::terminal::{Picture, Settled, Terminal};
use crate::native::wait::{Condition, Pattern};
use crate::output::{self, call_line};
use crate::protocol::{
    self, Addressed, AttachEvent, AttachInput, Attached, BatchOutcome, BatchStep, Call, Checked, Closed, CommandOutput,
    CommandRecord, Dashboard, DashboardStopped, Delivered, ErrorBody, Expect, Export, History, HumanInput, Input,
    Marked, Matches, Mirror, Printed, RecordingFile, Reply, Request, Response, RowRange, SavedRecording, Screenshot,
    SessionInfo, Sessions, Shell, Shot, Size, Snapshot, SnapshotOptions, Style, TabId, TabRef, Target, TextMatch, Wait,
    Waited,
};

const CONTROL_HANDED_BACK: &str = "a human to hand back control";

#[derive(Default)]
struct Daemon {
    sessions: Mutex<BTreeMap<String, Session>>,
    restarting: Mutex<()>,
    activity: Mutex<Activity>,
    quiet: Condvar,
    folders: Mutex<HashMap<String, Weak<SessionFolder>>>,
    changes: Changes,
    dashboard: Mutex<Option<dashboard::Server>>,
    me: Weak<Daemon>,
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

struct Logged {
    session: String,
    tab: Option<TabId>,
    raw: bool,
    folder: Option<Arc<SessionFolder>>,
    pending: Option<Pending>,
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

    let daemon = Arc::new_cyclic(|me| Daemon { me: me.clone(), ..Daemon::default() });
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
        let request = match call.request {
            Request::Attach { target, read_only } if busy.is_some() => {
                return self.attach(stream, &target, read_only);
            }
            request => request,
        };
        let stop = matches!(request, Request::Stop);
        let logged = call.typed.and_then(|typed| self.about_to_handle(typed, &request));
        let handled = match &busy {
            Some(_) => self.handle(request, logged.as_ref().map(|logged| logged.typed.as_slice())),
            None => Err(stopping()),
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

    fn handle(&self, request: Request, typed: Option<&[String]>) -> Result<Reply, ErrorBody> {
        match request {
            Request::Open { session, program, args, size, cwd, env, wait, timeout_ms } => {
                self.open(session, pty::Command { program, args, cwd, env, size }, wait.as_ref(), timeout_ms)
            }
            Request::Snapshot { target, options } => {
                let (id, terminal) = self.target(&target)?;
                terminal.snapshot_with(&target.session, &id.to_string(), &options).map(Reply::Snapshot).map_err(failure)
            }
            Request::Close { target, all } => {
                let closed = if all { self.close_all() } else { vec![self.close(&target)?] };
                Ok(Reply::Closed(Closed { closed }))
            }
            Request::Resize { target, size, wait, timeout_ms } => self.resize(&target, size, &wait, timeout_ms),
            Request::Restart { target, wait, timeout_ms } => self.restart(&target, &wait, timeout_ms),
            Request::Sessions => Ok(Reply::Sessions(self.list())),
            Request::Tabs { session } => {
                let sessions = self.sessions.lock().unwrap();
                Ok(Reply::Tabs(
                    sessions.get(&session).map_or_else(|| SessionInfo::empty(&session), |s| s.info(&session)),
                ))
            }
            Request::WaitControl { target, timeout_ms, assert } => self.wait_control(target, timeout_ms, assert),
            Request::Attach { .. } => Err(failure(
                "attach shows the tab in your terminal, so it runs on its own, not in batch or from the dashboard"
                    .into(),
            )),
            Request::SwitchTab { session, tab } => {
                let mut sessions = self.sessions.lock().unwrap();
                let entry = sessions
                    .get_mut(&session)
                    .filter(|s| s.tabs.contains_key(&tab))
                    .ok_or_else(|| no_tab(&session, tab))?;
                entry.current = Some(tab);
                Ok(Reply::Tabs(entry.info(&session)))
            }
            Request::Input { target, input, wait, snapshot, screenshot, timeout_ms } => {
                let after = After { wait: wait.as_ref(), snapshot, screenshot: screenshot.as_ref() };
                self.input(target, &input, after, timeout_ms, typed)
            }
            Request::Screenshot { target, shot } => {
                let (id, terminal) = self.target(&target)?;
                self.screenshot(&target.session, &id.to_string(), &terminal, &shot, typed).map(Reply::Screenshot)
            }
            Request::DiffScreenshot { target, baseline, shot, threshold } => {
                let (id, terminal) = self.target(&target)?;
                screenshots::diff(&target.session, &id.to_string(), &terminal, baseline, &shot, threshold)
                    .map(Reply::ScreenshotDiff)
                    .map_err(error_body)
            }
            Request::Clip { target, clip } => {
                let (id, terminal) = self.target(&target)?;
                let session = &target.session;
                let folder = self.sessions.lock().unwrap().get(session).map(|entry| Arc::clone(&entry.folder));
                let folder = folder.ok_or_else(|| no_terminal(session))?;
                clips::cut(session, &id.to_string(), &terminal, &folder, &clip).map(Reply::Clip).map_err(error_body)
            }
            Request::Wait { target, wait: Wait::Elapsed { ms }, timeout_ms, assert } => {
                self.wait(target, &Wait::Elapsed { ms }, timeout_ms.max(ms), assert)
            }
            Request::Wait { target, wait, timeout_ms, assert } => self.wait(target, &wait, timeout_ms, assert),
            Request::Expect { target, expect } => self.expect(target, &expect),
            Request::Get { target, what } => {
                let (_, terminal) = self.target(&target)?;
                terminal.fact(what).map(Reply::Fact).map_err(failure)
            }
            Request::Find { target, text, regex, all } => {
                let (_, terminal) = self.target(&target)?;
                let matches = terminal.find(&text, regex, all).map_err(failure)?;
                Ok(Reply::Matches(Matches { matches }))
            }
            Request::Mark { target, name } => {
                let (_, terminal) = self.target(&target)?;
                let frame = terminal.mark(name.clone());
                Ok(Reply::Marked(Marked { mark: name, frame }))
            }
            Request::Output { target, since, raw } => {
                let (_, terminal) = self.target(&target)?;
                let output = terminal.printed(since.as_deref(), raw).map_err(failure)?;
                Ok(Reply::Printed(Printed { output }))
            }
            Request::DiffSnapshot { target, baseline } => {
                let (_, terminal) = self.target(&target)?;
                terminal.diff_snapshot(baseline).map(Reply::Diff).map_err(failure)
            }
            Request::RecordStart { target, path } => {
                let (id, terminal) = self.target(&target)?;
                let Target { session, .. } = target;
                let path = match path {
                    Some(path) => path,
                    None => home().map_err(error_body)?.join("recordings").join(format!(
                        "{}-{id}-{}.cast",
                        store::file_name(&session),
                        terminal.frame()
                    )),
                };
                terminal.recording().save_to(&path).map_err(error_body)?;
                self.changes.notify();
                Ok(Reply::Recording(RecordingFile { session, tab: id.to_string(), path }))
            }
            Request::RecordStop { target } => {
                let (id, terminal) = self.target(&target)?;
                let saved = terminal.recording().stop_saving().map_err(error_body)?;
                self.changes.notify();
                Ok(Reply::Saved(SavedRecording {
                    session: target.session,
                    tab: id.to_string(),
                    path: saved.path,
                    duration_ms: saved.duration.as_millis() as u64,
                    bytes: saved.bytes,
                }))
            }
            Request::TraceExport { session, path } => {
                let exported = home().and_then(|home| store::export(&home, &session, &path)).map_err(error_body)?;
                Ok(Reply::Exported(Export { session, path, files: exported.files, bytes: exported.bytes }))
            }
            Request::Shell { session, shell, program, size, cwd, env, wait, timeout_ms } => self.open_shell(
                session,
                shell,
                pty::Command { program, args: Vec::new(), cwd, env, size },
                wait.as_ref(),
                timeout_ms,
            ),
            Request::Exec { target, line, check, timeout_ms } => self.exec(target, &line, check, timeout_ms),
            Request::History { target, last, failed } => {
                let (id, terminal) = self.target(&target)?;
                let (session, tab) = (target.session, id.to_string());
                let mut commands = terminal.commands().ok_or_else(|| no_integration(&session, &tab, &terminal))?;
                let total = commands.len();
                if failed {
                    commands.retain(CommandRecord::failed);
                }
                if let Some(last) = last {
                    commands.drain(..commands.len().saturating_sub(last));
                }
                Ok(Reply::History(History { session, tab, total, commands }))
            }
            Request::HistoryShow { target, id } => {
                let (tab_id, terminal) = self.target(&target)?;
                let (session, tab) = (target.session, tab_id.to_string());
                if terminal.shell().is_none() {
                    return Err(no_integration(&session, &tab, &terminal));
                }
                let (record, output) = terminal.command(id).ok_or_else(|| {
                    failure(format!("no command {id} in {session} {tab}; list them with `agent-terminal history`"))
                })?;
                Ok(Reply::Command(CommandOutput { session, tab, record, output, check: false }))
            }
            Request::Stop => {
                self.shut_down();
                Ok(Reply::Stopped)
            }
            Request::Batch { steps, bail } => Ok(Reply::Batch(self.batch(steps, bail))),
            Request::DashboardStart { port } => self.start_dashboard(port).map(Reply::Dashboard),
            Request::DashboardStop => Ok(Reply::DashboardStopped(DashboardStopped { stopped: self.stop_dashboard() })),
        }
    }

    fn list(&self) -> Sessions {
        let sessions = self.sessions.lock().unwrap();
        Sessions { sessions: sessions.iter().map(|(name, session)| session.info(name)).collect() }
    }

    fn start_dashboard(&self, port: u16) -> Result<Dashboard, ErrorBody> {
        let mut running = self.dashboard.lock().unwrap();
        if let Some(server) = running.as_ref() {
            if port == 0 || port == server.port() {
                return Ok(server.info());
            }
            return Err(failure(format!(
                "the dashboard already runs at {}; stop it first with `agent-terminal dashboard stop`",
                server.info().url
            )));
        }
        let backend: Arc<dyn Backend> =
            self.me.upgrade().ok_or_else(|| failure("the daemon is shutting down".into()))?;
        let server = dashboard::Server::start(port, backend).map_err(error_body)?;
        let info = server.info();
        *running = Some(server);
        Ok(info)
    }

    fn stop_dashboard(&self) -> bool {
        let server = self.dashboard.lock().unwrap().take();
        server.is_some()
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
        let handled = self.handle(request, Some(args));
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
        let time = SystemTime::now();
        let folder = sessions.get(session).map(|s| Arc::clone(&s.folder));
        let pending = folder.as_ref().map(|folder| folder.begin(time, tab.map(|id| id.to_string()), &typed));
        Some(Logged {
            session: session.to_string(),
            tab,
            raw: matches!(request, Request::Input { input: Input::Send { .. }, .. }),
            folder,
            pending,
            typed,
            time,
            started: Instant::now(),
        })
    }

    fn log(&self, logged: Logged, handled: &Result<Reply, ErrorBody>) {
        let mut entry = ActivityEntry::at(logged.time, &logged.typed, logged.started.elapsed());
        entry.raw = logged.raw;
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
        entry.tab = shown.map(|snapshot| snapshot.tab.clone()).or_else(|| logged.tab.map(|id| id.to_string()));
        entry.frame = shown.map(|snapshot| snapshot.frame).or_else(|| {
            let (_, terminal) = self.target(&Target::tab(&logged.session, logged.tab?)).ok()?;
            Some(terminal.frame())
        });
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
        entry.screenshots = match handled {
            Ok(Reply::Screenshot(screenshot)) => vec![screenshot.path.clone()],
            Ok(Reply::ScreenshotDiff(diff)) => vec![diff.path.clone()],
            Ok(Reply::Delivered(delivered)) => delivered.screenshot.iter().map(|shot| shot.path.clone()).collect(),
            Ok(_) | Err(_) => Vec::new(),
        };
        if let Ok(Reply::Clip(clip)) = handled {
            entry.clips = vec![clip.path.clone()];
        }
        match logged.pending {
            Some(pending) => pending.log(&entry),
            None => folder.log(&entry),
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
        self.stop_dashboard();
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
        let (tab, terminal) = self.open_tab(&session, command, None)?;
        first_screen(&session, tab, &terminal, wait, timeout_ms)
    }

    fn open_shell(
        &self,
        session: String,
        shell: Shell,
        command: pty::Command,
        wait: Option<&Wait>,
        timeout_ms: u64,
    ) -> Result<Reply, ErrorBody> {
        let command = shell::prepare(shell, command).map_err(error_body)?;
        let (tab, terminal) = self.open_tab(&session, command, Some(shell))?;
        first_screen(&session, tab, &terminal, wait, timeout_ms).map_err(|mut error| {
            if matches!(wait, Some(Wait::Command)) && error.code == crate::exit::TIMEOUT {
                error.message = format!(
                    "timed out after {timeout_ms} ms waiting for the first prompt of {} in {session} {tab}; its startup files may be waiting for input or the shell integration did not load (read the screen with `agent-terminal snapshot`, check with `agent-terminal doctor`)",
                    terminal.program
                );
            }
            error
        })
    }

    fn open_tab(
        &self,
        session: &str,
        command: pty::Command,
        shell: Option<Shell>,
    ) -> Result<(TabId, Arc<Terminal>), ErrorBody> {
        let spawned = pty::spawn(command.clone()).map_err(error_body)?;
        let folder = match self.folder(session) {
            Ok(folder) => folder,
            Err(e) => {
                spawned.abandon();
                return Err(error_body(e));
            }
        };
        let tab = folder.next_tab();
        let recording = folder.record(&tab.to_string(), &command);
        let command_log = folder.command_log(&tab.to_string());
        let terminal = Terminal::start(command, shell, spawned, recording, command_log, self.changes.clone(), 1);
        {
            let mut sessions = self.sessions.lock().unwrap();
            let entry = sessions.entry(session.to_string()).or_insert_with(|| Session::new(Arc::clone(&folder)));
            let control = Control::new(self.changes.clone());
            entry.tabs.insert(tab, Tab { terminal: Arc::clone(&terminal), control });
            entry.current = Some(tab);
        }
        self.changes.notify();
        Ok((tab, terminal))
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

    fn resize(&self, target: &Target, size: Size, wait: &Wait, timeout_ms: u64) -> Result<Reply, ErrorBody> {
        let (tab, terminal) = self.target(target)?;
        let session = &target.session;
        terminal.resize(size).map_err(|e| error_body(e.context(format!("{session} {tab}"))))?;
        settle(session, tab, &terminal, wait, timeout_ms)
    }

    fn restart(&self, target: &Target, wait: &Wait, timeout_ms: u64) -> Result<Reply, ErrorBody> {
        let restarting = self.restarting.lock().unwrap();
        let (tab, old) = self.target(target)?;
        let session = &target.session;
        if !old.stop(Duration::from_millis(timeout_ms)) {
            return Err(ErrorBody {
                code: crate::exit::TIMEOUT,
                message: format!(
                    "{} in {session} {tab} did not stop within {timeout_ms} ms, so it was not restarted",
                    old.program
                ),
                snapshot: None,
            });
        }
        let command = old.rerun();
        let spawned = pty::spawn(command.clone()).map_err(error_body)?;
        let recording = old.recording();
        recording.restart();
        let first_command = old.next_command_id();
        let terminal = Terminal::start(
            command,
            old.shell(),
            spawned,
            recording,
            old.command_log(),
            self.changes.clone(),
            first_command,
        );
        {
            let mut sessions = self.sessions.lock().unwrap();
            match sessions.get_mut(session).and_then(|s| s.tabs.get_mut(&tab)) {
                Some(slot) if Arc::ptr_eq(&slot.terminal, &old) => slot.terminal = Arc::clone(&terminal),
                _ => {
                    terminal.kill();
                    return Err(no_tab(session, tab));
                }
            }
        }
        self.changes.notify();
        drop(restarting);
        settle(session, tab, &terminal, wait, timeout_ms)
    }

    fn input(
        &self,
        target: Target,
        input: &Input,
        after: After<'_>,
        timeout_ms: u64,
        typed: Option<&[String]>,
    ) -> Result<Reply, ErrorBody> {
        let (id, Tab { terminal, control }) = self.tab(&target)?;
        let (session, tab) = (target.session, id.to_string());
        let condition = after.wait.map(condition).transpose()?;
        let delivery = {
            let _turn = control.agent().ok_or_else(|| human_has_control(&session, &tab))?;
            terminal.input(input).map_err(|e| error_body(e.context(format!("{session} {tab}"))))?
        };
        if let Some((wait, condition)) = after.wait.zip(condition) {
            let condition = condition.counted_from(Instant::now());
            wait_for(&terminal, &session, &tab, wait, condition, timeout_ms)?;
        }
        let screenshot = after
            .screenshot
            .map(|shot| self.screenshot(&session, &tab, &terminal, shot, typed))
            .transpose()?
            .map(Box::new);
        let snapshot = after.snapshot.then(|| Box::new(terminal.snapshot(&session, &tab)));
        Ok(Reply::Delivered(Delivered { session, tab, delivery, screenshot, snapshot }))
    }

    fn screenshot(
        &self,
        session: &str,
        tab: &str,
        terminal: &Terminal,
        shot: &Shot,
        typed: Option<&[String]>,
    ) -> Result<Screenshot, ErrorBody> {
        let taken = screenshots::take(session, tab, terminal, shot, typed.map(call_line)).map_err(error_body)?;
        let folder = self.sessions.lock().unwrap().get(session).map(|entry| Arc::clone(&entry.folder));
        if let Some((entry, folder)) = taken.listed.zip(folder) {
            folder.keep_screenshot(&entry);
        }
        Ok(taken.screenshot)
    }

    fn wait(&self, target: Target, wait: &Wait, timeout_ms: u64, assert: bool) -> Result<Reply, ErrorBody> {
        let (id, terminal) = self.target(&target)?;
        let (session, tab) = (target.session, id.to_string());
        if matches!(wait, Wait::Command) && terminal.shell().is_none() {
            return Err(no_integration(&session, &tab, &terminal));
        }
        let condition = condition(wait)?;
        let started = Instant::now();
        let frame = wait_for(&terminal, &session, &tab, wait, condition, timeout_ms).map_err(|error| match error {
            ErrorBody { code: crate::exit::TIMEOUT, snapshot, .. } if assert => ErrorBody {
                code: crate::exit::ASSERTION,
                message: format!("{} within {timeout_ms} ms in {session} {tab}", not_met(wait)),
                snapshot,
            },
            error => error,
        })?;
        let waited_ms = started.elapsed().as_millis() as u64;
        let exit_code = match wait {
            Wait::Exit { .. } => terminal.exit_status().flatten(),
            Wait::Command => terminal.last_exit_code(),
            _ => None,
        };
        Ok(Reply::Waited(Waited { session, tab, until: describe(wait), waited_ms, frame, exit_code }))
    }

    fn wait_control(&self, target: Target, timeout_ms: u64, assert: bool) -> Result<Reply, ErrorBody> {
        let (id, Tab { terminal, control }) = self.tab(&target)?;
        let (session, tab) = (target.session, id.to_string());
        let started = Instant::now();
        match control.wait_for_agent(Duration::from_millis(timeout_ms)) {
            Handed::Back => {
                let frame =
                    self.target(&Target::tab(&session, id)).map_or_else(|_| terminal.frame(), |(_, now)| now.frame());
                let waited_ms = started.elapsed().as_millis() as u64;
                let until = CONTROL_HANDED_BACK.to_string();
                Ok(Reply::Waited(Waited { session, tab, until, waited_ms, frame, exit_code: None }))
            }
            Handed::TimedOut if assert => Err(ErrorBody {
                code: crate::exit::ASSERTION,
                message: format!("a human did not hand back control of {session} {tab} within {timeout_ms} ms"),
                snapshot: None,
            }),
            Handed::TimedOut => Err(ErrorBody {
                code: crate::exit::TIMEOUT,
                message: format!(
                    "timed out after {timeout_ms} ms waiting for {CONTROL_HANDED_BACK} of {session} {tab}; wait longer with --timeout <ms>"
                ),
                snapshot: None,
            }),
            Handed::Closed => Err(failure(format!("stopped waiting for control: {session} {tab} was closed"))),
        }
    }

    fn attach(&self, mut stream: UnixStream, target: &Target, read_only: bool) {
        let session = target.session.as_str();
        let taken = self.tab(target).map(|(id, tab)| {
            let hold = (!read_only).then(|| tab.control.take());
            (id, tab, hold)
        });
        let (id, attached, hold) = match taken {
            Ok(taken) => taken,
            Err(error) => {
                if let Err(e) = protocol::write(&mut stream, &Response::Err(error)) {
                    eprintln!("attach: refusing the attach to {session} failed: {e:#}");
                }
                return;
            }
        };
        let reply = Reply::Attached(Attached {
            session: session.to_string(),
            tab: id.to_string(),
            program: attached.terminal.program.clone(),
            read_only,
        });
        if let Err(e) = protocol::write(&mut stream, &Response::Ok(reply)) {
            eprintln!("attach: answering the attach to {session} {id} failed: {e:#}");
            return;
        }
        if let Err(e) = self.mirror(&stream, session, id, &attached.control, hold.as_ref()) {
            eprintln!("attach: mirroring {session} {id} ended: {e:#}");
        }
        if let Err(e) = stream.shutdown(std::net::Shutdown::Both) {
            eprintln!("attach: closing the connection to {session} {id} failed: {e}");
        }
    }

    fn mirror(
        &self,
        stream: &UnixStream,
        session: &str,
        id: TabId,
        control: &Arc<Control>,
        hold: Option<&Hold>,
    ) -> Result<()> {
        let mut writer = stream.try_clone().context("share the attach connection")?;
        let reader = stream.try_clone().context("share the attach connection")?;
        let (inputs, mut received) = tokio::sync::mpsc::unbounded_channel();
        std::thread::spawn(move || read_attach_inputs(reader, inputs));
        let runtime = tokio::runtime::Builder::new_current_thread().build().context("start the attach runtime")?;
        runtime.block_on(async {
            let mut lifecycle = self.changes.subscribe();
            let mut watched: Option<(Arc<Terminal>, watch::Receiver<u64>)> = None;
            let mut sent = None;
            loop {
                lifecycle.borrow_and_update();
                let Some(terminal) = self.terminal_under(session, id, control) else {
                    return protocol::write(&mut writer, &AttachEvent::Closed);
                };
                let replaced = !watched.as_ref().is_some_and(|(watching, _)| Arc::ptr_eq(watching, &terminal));
                let (_, frames) = match &mut watched {
                    Some(watching) if !replaced => watching,
                    stale => stale.insert((Arc::clone(&terminal), terminal.frames())),
                };
                frames.borrow_and_update();
                let picture = terminal.picture();
                let shown = (picture.frame, picture.exit, picture.size);
                if sent != Some(shown) {
                    protocol::write(&mut writer, &AttachEvent::Screen(mirrored(&terminal.program, picture)))?;
                    sent = Some(shown);
                }
                tokio::select! {
                    changed = lifecycle.changed() => changed?,
                    changed = frames.changed() => changed?,
                    input = received.recv() => match (input, hold) {
                        (Some(AttachInput::Human(input)), Some(hold)) => {
                            if let Err(refused) = self.type_as_human(session, id, hold, &input) {
                                eprintln!("attach: {}", refused.message);
                            }
                        }
                        (Some(AttachInput::Human(_)), None) => {}
                        (Some(AttachInput::Detach), _) => return protocol::write(&mut writer, &AttachEvent::Detached),
                        (None, _) => return Ok(()),
                    },
                }
            }
        })
    }

    fn terminal_of(&self, session: &str, id: TabId) -> Option<Arc<Terminal>> {
        self.sessions
            .lock()
            .unwrap()
            .get(session)
            .and_then(|entry| entry.tabs.get(&id))
            .map(|tab| Arc::clone(&tab.terminal))
    }

    fn terminal_under(&self, session: &str, id: TabId, control: &Arc<Control>) -> Option<Arc<Terminal>> {
        let sessions = self.sessions.lock().unwrap();
        let tab = sessions.get(session).and_then(|entry| entry.tabs.get(&id))?;
        Arc::ptr_eq(&tab.control, control).then(|| Arc::clone(&tab.terminal))
    }

    fn exec(&self, target: Target, line: &str, check: bool, timeout_ms: u64) -> Result<Reply, ErrorBody> {
        let (id, Tab { terminal, control }) = self.tab(&target)?;
        let (session, tab) = (target.session, id.to_string());
        if terminal.shell().is_none() {
            return Err(no_integration(&session, &tab, &terminal));
        }
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let left = || deadline.saturating_duration_since(Instant::now());
        let submitted = loop {
            let submitted = {
                let _turn = control.agent().ok_or_else(|| human_has_control(&session, &tab))?;
                terminal.submit(line).map_err(|e| error_body(e.context(format!("{session} {tab}"))))?
            };
            if let Some(submitted) = submitted {
                break submitted;
            }
            match terminal.wait(Condition::Command, left()) {
                Settled::Met { .. } => continue,
                Settled::Exited => {
                    return Err(exited_while_waiting(&Wait::Command, terminal.snapshot(&session, &tab)));
                }
                Settled::TimedOut => {
                    let waiting = match terminal.running_command() {
                        Some(running) => format!("`{running}` to finish"),
                        None => "the shell prompt".to_string(),
                    };
                    let message = format!(
                        "timed out after {timeout_ms} ms waiting for {waiting} in {session} {tab}, so `{line}` did not run (read the screen with `agent-terminal snapshot`, interrupt a command with `agent-terminal signal INT`)"
                    );
                    return Err(with_screen(crate::exit::TIMEOUT, message, terminal.snapshot(&session, &tab)));
                }
            }
        };
        match terminal.wait(Condition::PromptAfter(submitted.prompts), left()) {
            Settled::Met { .. } => {}
            Settled::Exited => return Err(exited_while_waiting(&Wait::Command, terminal.snapshot(&session, &tab))),
            Settled::TimedOut => {
                let waited =
                    format!("timed out after {timeout_ms} ms waiting for `{line}` to finish in {session} {tab}");
                let id = submitted.first_id;
                let message = match terminal.command(id) {
                    Some(_) => format!(
                        "{waited}; it is still running as command {id} (wait for it with `agent-terminal wait --command`, read its output with `agent-terminal history show {id}`, interrupt it with `agent-terminal signal INT`)"
                    ),
                    None => format!(
                        "{waited}; the shell has not started a command for it, so the line may be unfinished (read the screen with `agent-terminal snapshot`, cancel the line with `agent-terminal press Ctrl+c`)"
                    ),
                };
                return Err(with_screen(crate::exit::TIMEOUT, message, terminal.snapshot(&session, &tab)));
            }
        }
        let (record, output) = terminal.command(submitted.first_id).ok_or_else(|| {
            let message = format!("the shell in {session} {tab} ran no command for `{line}`; the screen shows why");
            with_screen(crate::exit::ERROR, message, terminal.snapshot(&session, &tab))
        })?;
        Ok(Reply::Command(CommandOutput { session, tab, record, output, check }))
    }

    fn expect(&self, target: Target, expect: &Expect) -> Result<Reply, ErrorBody> {
        let (id, terminal) = self.target(&target)?;
        let (session, tab) = (target.session, id.to_string());
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

    fn close(&self, target: &Target) -> Result<TabRef, ErrorBody> {
        let Target { session, tab } = target;
        let mut sessions = self.sessions.lock().unwrap();
        let missing = || tab.map_or_else(|| no_terminal(session), |id| no_tab(session, id));
        let entry = sessions.get_mut(session).ok_or_else(missing)?;
        let id = tab.or(entry.current).ok_or_else(missing)?;
        let closing = entry.tabs.remove(&id).ok_or_else(missing)?;
        closing.terminal.kill();
        closing.control.close();
        if entry.current == Some(id) {
            entry.current = entry.tabs.keys().next_back().copied();
        }
        if entry.tabs.is_empty() {
            sessions.remove(session);
        }
        drop(sessions);
        self.changes.notify();
        Ok(TabRef { session: session.to_string(), tab: id.to_string() })
    }

    fn close_all(&self) -> Vec<TabRef> {
        let sessions = std::mem::take(&mut *self.sessions.lock().unwrap());
        self.changes.notify();
        let mut closed = Vec::new();
        for (name, session) in sessions {
            for (id, closing) in session.tabs {
                closing.terminal.kill();
                closing.control.close();
                closed.push(TabRef { session: name.clone(), tab: id.to_string() });
            }
        }
        closed
    }

    fn target(&self, target: &Target) -> Result<(TabId, Arc<Terminal>), ErrorBody> {
        self.tab(target).map(|(id, tab)| (id, tab.terminal))
    }

    fn tab(&self, target: &Target) -> Result<(TabId, Tab), ErrorBody> {
        let Target { session, tab } = target;
        let sessions = self.sessions.lock().unwrap();
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
        self.sessions.lock().unwrap().get(session).map(|entry| Arc::clone(&entry.folder))
    }

    fn changes(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    fn take_control(&self, session: &str, tab: TabId) -> Result<Hold, ErrorBody> {
        self.tab(&Target::tab(session, tab)).map(|(_, tab)| tab.control.take())
    }

    fn holds(&self, session: &str, tab: TabId, hold: &Hold) -> bool {
        let sessions = self.sessions.lock().unwrap();
        sessions.get(session).and_then(|entry| entry.tabs.get(&tab)).is_some_and(|tab| hold.holds(&tab.control))
    }

    fn type_as_human(&self, session: &str, id: TabId, hold: &Hold, input: &HumanInput) -> Result<(), ErrorBody> {
        let time = SystemTime::now();
        let started = Instant::now();
        let (folder, terminal) = {
            let sessions = self.sessions.lock().unwrap();
            let held = sessions.get(session).and_then(|s| {
                let tab = s.tabs.get(&id).filter(|tab| hold.holds(&tab.control))?;
                Some((Arc::clone(&s.folder), Arc::clone(&tab.terminal)))
            });
            held.ok_or_else(|| failure(format!("{session} {id} was closed")))?
        };
        let delivered = terminal.input(&input.input()).map(drop).map_err(|e| {
            let mut error = error_body(e);
            error.message = format!("{session} {id}: {}", error.message);
            error
        });
        let mut entry = ActivityEntry::at(time, &input.call(), started.elapsed());
        entry.tab = Some(id.to_string());
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

struct After<'a> {
    wait: Option<&'a Wait>,
    snapshot: bool,
    screenshot: Option<&'a Shot>,
}

fn first_screen(
    session: &str,
    tab: TabId,
    terminal: &Terminal,
    wait: Option<&Wait>,
    timeout_ms: u64,
) -> Result<Reply, ErrorBody> {
    match wait {
        Some(wait) => settle(session, tab, terminal, wait, timeout_ms),
        None => Ok(Reply::Snapshot(terminal.snapshot(session, &tab.to_string()))),
    }
}

fn settle(session: &str, tab: TabId, terminal: &Terminal, wait: &Wait, timeout_ms: u64) -> Result<Reply, ErrorBody> {
    let tab = tab.to_string();
    let settled = terminal.wait(condition(wait)?.counted_from(Instant::now()), Duration::from_millis(timeout_ms));
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

fn human_has_control(session: &str, tab: &str) -> ErrorBody {
    ErrorBody {
        code: crate::exit::HUMAN_CONTROL,
        message: format!("A human has control of {session} {tab}. Run `agent-terminal wait --control`."),
        snapshot: None,
    }
}

fn mirrored(program: &str, picture: Picture) -> Mirror {
    Mirror {
        program: program.to_string(),
        frame: picture.frame,
        cols: picture.size.cols,
        rows: picture.size.rows,
        lines: picture.rows,
        cursor: [picture.cursor.0, picture.cursor.1],
        cursor_visible: picture.cursor_visible,
        exited: picture.exit.is_some(),
        exit_code: picture.exit.flatten(),
    }
}

fn read_attach_inputs(mut reader: UnixStream, inputs: tokio::sync::mpsc::UnboundedSender<AttachInput>) {
    loop {
        match protocol::read::<AttachInput>(&mut reader) {
            Ok(Some(input)) => {
                if inputs.send(input).is_err() {
                    return;
                }
            }
            Ok(None) => return,
            Err(e) => {
                eprintln!("attach: reading the keys of an attached terminal failed: {e:#}");
                return;
            }
        }
    }
}

fn with_screen(code: i32, message: String, snapshot: Snapshot) -> ErrorBody {
    ErrorBody { code, message, snapshot: Some(Box::new(snapshot)) }
}

fn no_integration(session: &str, tab: &str, terminal: &Terminal) -> ErrorBody {
    failure(format!(
        "{session} {tab} runs {} without shell integration, so it keeps no command history; exec, history and wait --command work in a tab opened with `agent-terminal shell`",
        terminal.program
    ))
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
        Wait::Command => "the shell command to finish".to_string(),
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

fn not_met(wait: &Wait) -> String {
    match wait {
        Wait::Stable { quiet_ms } => format!("the screen did not stay unchanged for {quiet_ms} ms"),
        Wait::Idle { quiet_ms } => format!("the program did not stop printing for {quiet_ms} ms"),
        Wait::Elapsed { ms } => format!("{ms} ms did not pass"),
        Wait::Exit { code: None } => "the program did not exit".to_string(),
        Wait::Exit { code: Some(code) } => format!("the program did not exit with code {code}"),
        Wait::Cursor { row, col } => format!("the cursor did not reach {row},{col}"),
        Wait::Frame { frame } => format!("the screen did not reach frame {frame}"),
        Wait::Command => "the shell command did not finish".to_string(),
        Wait::Text(text) => describe_text(text, "did not appear", "nothing matched"),
        Wait::Gone(text) => describe_text(text, "did not disappear", "still a match for"),
    }
}

fn stopping() -> ErrorBody {
    failure("the daemon is stopping because it was idle; run the command again to start a new one".into())
}

fn no_terminal(session: &str) -> ErrorBody {
    ErrorBody {
        code: crate::exit::ERROR,
        message: format!("no terminal is open in session {session}; start one with `agent-terminal open <program>`"),
        snapshot: None,
    }
}

fn no_tab(session: &str, id: TabId) -> ErrorBody {
    failure(format!("no tab {id} in session {session}; list tabs with `agent-terminal tab`"))
}

fn failure(message: String) -> ErrorBody {
    ErrorBody { code: crate::exit::ERROR, message, snapshot: None }
}

fn error_body(error: anyhow::Error) -> ErrorBody {
    let code = error.downcast_ref::<crate::Error>().map_or(crate::exit::ERROR, |e| e.code);
    ErrorBody { code, message: format!("{error:#}"), snapshot: None }
}
