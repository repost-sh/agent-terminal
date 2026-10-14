use std::io::{Read, Write};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use tokio::sync::watch;

use crate::key::{Key, KeyCode, Mods};
use crate::native::changes::Changes;
use crate::native::commands::{Change, Commands, Point, Submitted};
use crate::native::keyboard;
use crate::native::mouse::{self, Action};
use crate::native::output_log::{Offsets, Output, OutputLog};
use crate::native::pty::{self, Killer, Signaller, Spawned};
use crate::native::recording::Recording;
use crate::native::refs::{self, Found, Grid};
use crate::native::screen::{Screen, ShellReport};
use crate::native::store::CommandLog;
use crate::native::wait::{self, Condition, Moment, Verdict};
use crate::protocol::{
    Baseline, Cell, ClickTarget, CommandRecord, Cursor, Delivery, DiffBase, Direction, Exit, Fact, Get, Input, Match,
    Modes, MouseButton, MouseEvent, Ref, RowChange, RowRange, ScrollBy, Scrollback, Shell, Size, Snapshot,
    SnapshotDiff, SnapshotOptions, TabInfo,
};

pub struct Terminal {
    pub program: String,
    pid: Option<u32>,
    cwd: String,
    started: SystemTime,
    state: Mutex<State>,
    changed: Condvar,
    input_turn: Mutex<()>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn Killer>>,
    master: pty::Master,
    signaller: Box<dyn Signaller>,
    command: pty::Command,
    shell: Option<Shell>,
    recording: Arc<Recording>,
    command_log: CommandLog,
    frames: Changes,
    history: Changes,
    lifecycle: Changes,
}

pub struct Setup {
    pub command: pty::Command,
    pub pid: Option<u32>,
    pub cwd: String,
    pub size: Size,
    pub modes: Modes,
    pub started: SystemTime,
    pub exit: Option<Option<i32>>,
}

pub struct Picture {
    pub frame: u64,
    pub size: Size,
    pub rows: Vec<String>,
    pub cursor: (u16, u16),
    pub cursor_visible: bool,
    pub alt_screen: bool,
    pub mouse: Option<&'static str>,
    pub bracketed_paste: bool,
    pub exit: Option<Option<i32>>,
}

struct State {
    screen: Screen,
    frame: u64,
    look: u64,
    last_change: Instant,
    last_output: Instant,
    exit: Option<Option<i32>>,
    log: OutputLog,
    last_shown: Option<Shown>,
    listed: Option<Listed>,
    mouse_held: Option<MouseButton>,
    watches: Vec<Watch>,
    next_watch: u64,
    commands: Option<Commands>,
}

struct Listed {
    frame: u64,
    refs: Vec<Found>,
}

struct Shown {
    frame: u64,
    lines: Vec<String>,
}

struct Watch {
    id: u64,
    condition: Arc<Condition>,
    met: Option<u64>,
}

impl State {
    fn moment(&self) -> Moment<'_> {
        let prompt = self.commands.as_ref().map(Commands::prompt);
        Moment::new(&self.screen, self.frame, self.last_change, self.last_output, self.exit.is_some(), prompt)
    }

    fn refresh(&mut self, now: Instant) {
        self.advance(now);
        self.publish();
    }

    fn advance(&mut self, now: Instant) {
        let look = self.screen.fingerprint();
        if look != self.look {
            self.look = look;
            self.frame += 1;
            self.last_change = now;
        }
    }

    fn publish(&mut self) {
        let State { screen, frame, last_change, last_output, exit, watches, commands, .. } = self;
        if watches.iter().all(|watch| watch.met.is_some()) {
            return;
        }
        let prompt = commands.as_ref().map(Commands::prompt);
        let moment = Moment::new(screen, *frame, *last_change, *last_output, exit.is_some(), prompt);
        for watch in watches.iter_mut().filter(|watch| watch.met.is_none()) {
            if let Verdict::Met = watch.condition.judge(&moment) {
                watch.met = Some(*frame);
            }
        }
    }
}

pub struct Capture {
    pub grid: Grid,
    pub rows: RowRange,
    pub frame: u64,
    pub refs: Option<Vec<Ref>>,
}

pub enum Settled {
    Met { frame: u64 },
    Exited,
    TimedOut,
}

impl Terminal {
    pub fn start(
        command: pty::Command,
        shell: Option<Shell>,
        spawned: Spawned,
        recording: Arc<Recording>,
        command_log: CommandLog,
        lifecycle: Changes,
    ) -> Arc<Self> {
        let program = display_name(&command.program);
        let size = command.size;
        let cwd = command.cwd.to_string_lossy().into_owned();
        let screen = Screen::new(size);
        let started = Instant::now();
        let commands = shell.map(|_| Commands::new(cwd.clone()));
        let terminal = Arc::new(Self {
            program,
            pid: spawned.pid,
            cwd,
            started: SystemTime::now(),
            state: Mutex::new(State {
                look: screen.fingerprint(),
                screen,
                frame: 0,
                last_change: started,
                last_output: started,
                exit: None,
                log: OutputLog::default(),
                last_shown: None,
                listed: None,
                mouse_held: None,
                watches: Vec::new(),
                next_watch: 0,
                commands,
            }),
            changed: Condvar::new(),
            input_turn: Mutex::new(()),
            writer: Mutex::new(spawned.writer),
            killer: Mutex::new(spawned.killer),
            master: spawned.master,
            signaller: spawned.signaller,
            command,
            shell,
            recording,
            command_log,
            frames: Changes::default(),
            history: Changes::default(),
            lifecycle,
        });

        let reading = Arc::clone(&terminal);
        let mut reader = spawned.reader;
        let output_done = std::thread::spawn(move || {
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => reading.output(&buf[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) if e.raw_os_error() == Some(libc::EIO) => break,
                    Err(e) => {
                        eprintln!("{}: reading program output failed: {e}", reading.program);
                        break;
                    }
                }
            }
        });

        let waiting = Arc::clone(&terminal);
        let wait = spawned.wait;
        std::thread::spawn(move || {
            let code =
                wait().inspect_err(|e| eprintln!("{}: could not read the exit status: {e:#}", waiting.program)).ok();
            if output_done.join().is_err() {
                eprintln!("{}: the output reader panicked", waiting.program);
            }
            waiting.state.lock().unwrap().exit = Some(code);
            waiting.changed.notify_all();
            waiting.frames.notify();
            waiting.lifecycle.notify();
        });

        terminal
    }

    fn output(&self, bytes: &[u8]) {
        let (replies, history_changed) = {
            let mut state = self.state.lock().unwrap();
            self.recording.output(bytes);
            let replies = state.screen.feed(bytes);
            let printed = state.screen.take_printed();
            let reports = state.screen.take_shell_reports();
            let frame = state.frame;
            let start = state.log.end();
            state.log.record(frame, bytes, printed.text.as_bytes(), printed.styled.as_bytes());
            let now = Instant::now();
            state.last_output = now;
            state.advance(now);
            let shown = state.frame;
            let mut history_changed = false;
            let State { commands, log, .. } = &mut *state;
            if let Some(commands) = commands {
                for (at, report) in reports {
                    let frame = if matches!(report, ShellReport::Started { .. }) { frame } else { shown };
                    let output = Offsets { text: start.text + at.text as u64, styled: start.styled + at.styled as u64 };
                    match commands.report(report, Point { output, frame }) {
                        Some(Change::Started) => history_changed = true,
                        Some(Change::Finished { id }) => {
                            history_changed = true;
                            if let Some((record, from, to)) = commands.record(id) {
                                self.command_log.append(&record, &log.text_between(from, to));
                            }
                        }
                        None => {}
                    }
                }
            }
            state.publish();
            (replies, history_changed)
        };
        self.changed.notify_all();
        self.frames.notify();
        if history_changed {
            self.history.notify();
        }
        if !replies.is_empty() {
            let mut writer = self.writer.lock().unwrap();
            if let Err(e) = writer.write_all(&replies).and_then(|()| writer.flush()) {
                eprintln!("{}: answering a terminal query failed: {e}", self.program);
            }
        }
    }

    pub fn snapshot(&self, session: &str, tab: &str) -> Snapshot {
        let mut state = self.state.lock().unwrap();
        let rows = state.screen.view().size.rows;
        self.render(&mut state, session, tab, RowRange { first: 1, last: rows }, &SnapshotOptions::default())
    }

    pub fn snapshot_with(&self, session: &str, tab: &str, options: &SnapshotOptions) -> Result<Snapshot, String> {
        let mut state = self.state.lock().unwrap();
        let range = row_range(state.screen.size(), options.rows)?;
        Ok(self.render(&mut state, session, tab, range, options))
    }

    pub fn capture(&self, rows: Option<RowRange>, refs: bool) -> Result<Capture, String> {
        let mut state = self.state.lock().unwrap();
        let rows = row_range(state.screen.size(), rows)?;
        let grid = state.screen.grid();
        let refs = refs.then(|| remember_refs(&mut state, &grid));
        Ok(Capture { grid, rows, frame: state.frame, refs })
    }

    pub fn text_rows(&self, rows: Option<RowRange>, ansi: bool) -> Result<(u64, Vec<String>), String> {
        let state = self.state.lock().unwrap();
        let rows = row_range(state.screen.size(), rows)?;
        Ok((state.frame, state.screen.rows(rows.first as i32 - 1..rows.last as i32, ansi)))
    }

    fn render(
        &self,
        state: &mut State,
        session: &str,
        tab: &str,
        range: RowRange,
        options: &SnapshotOptions,
    ) -> Snapshot {
        let view = state.screen.view();
        state.last_shown = Some(Shown { frame: state.frame, lines: view.lines.clone() });
        let refs = options.refs.then(|| list_refs(state));
        let history = state.screen.scrollback_len();
        let scrollback = match options.scrollback {
            Scrollback::None => None,
            Scrollback::All => Some(history),
            Scrollback::Last(n) => Some(n.min(history)),
        };
        Snapshot {
            session: session.to_string(),
            tab: tab.to_string(),
            program: self.program.clone(),
            cols: view.size.cols,
            rows: view.size.rows,
            alt_screen: view.alt_screen,
            mouse: view.mouse.map(|m| m.as_str().to_string()),
            cursor: [view.cursor.0 + 1, view.cursor.1 + 1],
            cursor_visible: view.cursor_visible,
            frame: state.frame,
            exited: state.exit.is_some(),
            exit_code: state.exit.flatten(),
            first_row: range.first,
            scrollback: scrollback.map(|n| state.screen.rows(-(n as i32)..0, options.ansi)),
            lines: state.screen.rows(range.first as i32 - 1..range.last as i32, options.ansi),
            refs,
        }
    }

    pub fn resize(&self, size: Size) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        if let Some(code) = state.exit {
            return Err(self.exited(code).into());
        }
        if !self.master.resize(size)? {
            return Err(self.exited(None).into());
        }
        state.screen.resize(size);
        self.recording.resize(size);
        state.refresh(Instant::now());
        drop(state);
        self.changed.notify_all();
        self.frames.notify();
        self.lifecycle.notify();
        Ok(())
    }

    pub fn frame(&self) -> u64 {
        self.state.lock().unwrap().frame
    }

    pub fn frames(&self) -> watch::Receiver<u64> {
        self.frames.subscribe()
    }

    pub fn picture(&self) -> Picture {
        let state = self.state.lock().unwrap();
        let view = state.screen.view();
        Picture {
            frame: state.frame,
            size: view.size,
            rows: state.screen.rows(0..view.size.rows as i32, true),
            cursor: (view.cursor.0 + 1, view.cursor.1 + 1),
            cursor_visible: view.cursor_visible,
            alt_screen: view.alt_screen,
            mouse: view.mouse.map(|mouse| mouse.as_str()),
            bracketed_paste: state.screen.keyboard_modes().bracketed_paste,
            exit: state.exit,
        }
    }

    pub fn setup(&self) -> Setup {
        let state = self.state.lock().unwrap();
        Setup {
            command: self.command.clone(),
            pid: self.pid,
            cwd: state.screen.reported_cwd().unwrap_or_else(|| self.cwd.clone()),
            size: state.screen.size(),
            modes: state.screen.modes(),
            started: self.started,
            exit: state.exit,
        }
    }

    pub fn recording(&self) -> Arc<Recording> {
        Arc::clone(&self.recording)
    }

    pub fn command_log(&self) -> CommandLog {
        self.command_log.clone()
    }

    pub fn history_changes(&self) -> watch::Receiver<u64> {
        self.history.subscribe()
    }

    pub fn rerun(&self) -> pty::Command {
        let size = self.state.lock().unwrap().screen.size();
        pty::Command { size, ..self.command.clone() }
    }

    pub fn stop(&self, timeout: Duration) -> bool {
        self.kill();
        let state = self.state.lock().unwrap();
        let (state, _) = self.changed.wait_timeout_while(state, timeout, |state| state.exit.is_none()).unwrap();
        state.exit.is_some()
    }

    pub fn info(&self, tab: &str) -> TabInfo {
        let state = self.state.lock().unwrap();
        let size = state.screen.size();
        TabInfo {
            tab: tab.to_string(),
            program: self.program.clone(),
            cols: size.cols,
            rows: size.rows,
            exited: state.exit.is_some(),
            exit_code: state.exit.flatten(),
        }
    }

    pub fn fact(&self, what: Get) -> Result<Fact, String> {
        let state = self.state.lock().unwrap();
        let view = state.screen.view();
        Ok(match what {
            Get::Text => {
                let shown = view.lines.iter().rposition(|line| !line.is_empty()).map_or(0, |i| i + 1);
                Fact::Text(view.lines[..shown].join("\n"))
            }
            Get::Cursor => {
                Fact::Cursor(Cursor { row: view.cursor.0 + 1, col: view.cursor.1 + 1, visible: view.cursor_visible })
            }
            Get::Size => Fact::Size(view.size),
            Get::Frame => Fact::Frame(state.frame),
            Get::Program => Fact::Program(self.program.clone()),
            Get::ExitCode => Fact::ExitCode(Exit { exited: state.exit.is_some(), code: state.exit.flatten() }),
            Get::Pid => Fact::Pid(self.pid),
            Get::Title => Fact::Title(state.screen.title()),
            Get::Cwd => Fact::Cwd(state.screen.reported_cwd().unwrap_or_else(|| self.cwd.clone())),
            Get::Modes => Fact::Modes(state.screen.modes()),
            Get::Cell { row, col } => Fact::Cell(cell_at(&state.screen, row, col)?),
        })
    }

    pub fn find(&self, text: &str, regex: bool, all: bool) -> Result<Vec<Match>, String> {
        let pattern = wait::compile(text, regex)?;
        let state = self.state.lock().unwrap();
        let first = if all { -(state.screen.scrollback_len() as i32) } else { 0 };
        let matches = matches_on(&state.screen, &pattern, first);
        if matches.is_empty() {
            let place = if all { "on the screen or in its scrollback" } else { "on the screen" };
            return Err(format!("{text:?} is not {place}"));
        }
        Ok(matches)
    }

    pub fn diff_snapshot(&self, baseline: Option<Baseline>) -> Result<SnapshotDiff, String> {
        let mut state = self.state.lock().unwrap();
        let current = Shown { frame: state.frame, lines: state.screen.view().lines };
        let (since, before) = match (baseline, state.last_shown.take()) {
            (Some(baseline), _) => (DiffBase::Baseline(baseline.name), baseline.lines),
            (None, Some(shown)) => (DiffBase::Frame(shown.frame), shown.lines),
            (None, None) => {
                return Err("nothing to compare with; take a snapshot first with `agent-terminal snapshot`".into());
            }
        };
        let rows = before.len().max(current.lines.len());
        let row = |lines: &[String], i: usize| lines.get(i).map_or("", String::as_str).to_string();
        let changes = (0..rows)
            .map(|i| RowChange { row: i as u16 + 1, before: row(&before, i), after: row(&current.lines, i) })
            .filter(|change| change.before != change.after)
            .collect();
        let frame = current.frame;
        state.last_shown = Some(current);
        Ok(SnapshotDiff { since, frame, changes })
    }

    pub fn mark(&self, name: String) -> u64 {
        let mut state = self.state.lock().unwrap();
        let frame = state.frame;
        state.log.mark(name, frame);
        frame
    }

    pub fn output_since(&self, raw: u64, styled: u64) -> Output {
        self.state.lock().unwrap().log.output_since(raw, styled)
    }

    pub fn printed(&self, since: Option<&str>, raw: bool) -> Result<String, String> {
        let state = self.state.lock().unwrap();
        state.log.since(since, raw, state.frame)
    }

    pub fn wait(&self, condition: Condition, timeout: Duration) -> Settled {
        let deadline = Instant::now() + timeout;
        let condition = Arc::new(condition);
        let mut state = self.state.lock().unwrap();
        let id = state.next_watch;
        state.next_watch += 1;
        state.watches.push(Watch { id, condition: Arc::clone(&condition), met: None });
        let settled = loop {
            if let Some(frame) = state.watches.iter().find(|watch| watch.id == id).and_then(|watch| watch.met) {
                break Settled::Met { frame };
            }
            let moment = state.moment();
            let wake = match condition.judge(&moment) {
                Verdict::Met => break Settled::Met { frame: state.frame },
                Verdict::Never => break Settled::Exited,
                Verdict::Pending(wake) => wake,
            };
            let now = moment.now;
            if now >= deadline {
                break Settled::TimedOut;
            }
            let until = wake.map_or(deadline, |wake| wake.min(deadline));
            state = self.changed.wait_timeout(state, until - now).unwrap().0;
        };
        state.watches.retain(|watch| watch.id != id);
        settled
    }

    pub fn input(&self, input: &Input) -> Result<Delivery> {
        let _turn = self.input_turn.lock().unwrap();
        match input {
            Input::Type { text, delay_ms } => {
                let keys = Key::for_text(text).map_err(anyhow::Error::msg)?;
                let mut bytes = 0;
                for (i, key) in keys.iter().enumerate() {
                    if i > 0 {
                        self.pause(Duration::from_millis(*delay_ms))?;
                    }
                    bytes += self.press(*key)?;
                }
                Ok(Delivery::Text { keys: keys.len(), bytes })
            }
            Input::Press { keys, repeat } => {
                let modes = self.state.lock().unwrap().screen.keyboard_modes();
                let encoded = keys
                    .iter()
                    .map(|key| keyboard::encode(*key, modes))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(anyhow::Error::msg)?;
                let mut bytes = 0;
                for _ in 0..*repeat {
                    for key in &encoded {
                        self.write(key)?;
                        bytes += key.len();
                    }
                }
                Ok(Delivery::Keys { keys: keys.len() * *repeat as usize, bytes })
            }
            Input::Paste { text } => {
                let modes = self.state.lock().unwrap().screen.keyboard_modes();
                let bytes = keyboard::paste(text, modes);
                self.write(&bytes)?;
                Ok(Delivery::Paste { bracketed: modes.bracketed_paste, bytes: bytes.len() })
            }
            Input::Send { bytes } => {
                self.write(bytes)?;
                Ok(Delivery::Raw { bytes: bytes.len() })
            }
            Input::Signal { signal } => {
                self.ensure_running()?;
                let process_group = self.signaller.signal(*signal)?;
                Ok(Delivery::Signal { signal: *signal, process_group })
            }
            Input::Click { target, button, double } => {
                let (row, col, bytes) = {
                    let mut state = self.running()?;
                    let reporting = state.screen.mouse_reporting().ok_or_else(no_mouse)?;
                    let (row, col) = click_target(&state, target)?;
                    let press =
                        mouse::encode(Action::Press(*button), row, col, reporting).map_err(anyhow::Error::msg)?;
                    let release =
                        mouse::encode(Action::Release(*button), row, col, reporting).map_err(anyhow::Error::msg)?;
                    state.mouse_held = None;
                    (row, col, [press, release].concat().repeat(if *double { 2 } else { 1 }))
                };
                self.write(&bytes)?;
                Ok(Delivery::Click {
                    target: target.clone(),
                    row,
                    col,
                    button: *button,
                    double: *double,
                    bytes: bytes.len(),
                })
            }
            Input::Mouse { event } => {
                let bytes = {
                    let mut state = self.running()?;
                    let reporting = state.screen.mouse_reporting().ok_or_else(no_mouse)?;
                    let (row, col) = event.at();
                    cell_at(&state.screen, row, col).map_err(anyhow::Error::msg)?;
                    let action = match *event {
                        MouseEvent::Move { .. } => Action::Move { held: state.mouse_held },
                        MouseEvent::Down { button, .. } => Action::Press(button),
                        MouseEvent::Up { button, .. } => Action::Release(button),
                        MouseEvent::Wheel { direction, .. } => Action::Wheel(direction),
                    };
                    let bytes = mouse::encode(action, row, col, reporting).map_err(anyhow::Error::msg)?;
                    match *event {
                        MouseEvent::Down { button, .. } => state.mouse_held = Some(button),
                        MouseEvent::Up { .. } => state.mouse_held = None,
                        MouseEvent::Move { .. } | MouseEvent::Wheel { .. } => {}
                    }
                    bytes
                };
                if !bytes.is_empty() {
                    self.write(&bytes)?;
                }
                Ok(Delivery::Mouse { event: *event, bytes: bytes.len() })
            }
            Input::Scroll { direction, count, keys } => {
                let (bytes, by) = {
                    let state = self.running()?;
                    match state.screen.mouse_reporting().filter(|_| !keys) {
                        Some(reporting) => {
                            let size = state.screen.size();
                            let (row, col) = (size.rows.div_ceil(2), size.cols.div_ceil(2));
                            let notch = mouse::encode(Action::Wheel(*direction), row, col, reporting)
                                .map_err(anyhow::Error::msg)?;
                            (notch, ScrollBy::Wheel)
                        }
                        None => {
                            let code = match direction {
                                Direction::Up => KeyCode::PageUp,
                                Direction::Down => KeyCode::PageDown,
                            };
                            let key = Key { code, mods: Mods::default() };
                            let page =
                                keyboard::encode(key, state.screen.keyboard_modes()).map_err(anyhow::Error::msg)?;
                            (page, ScrollBy::Keys)
                        }
                    }
                };
                let bytes = bytes.repeat(*count as usize);
                self.write(&bytes)?;
                Ok(Delivery::Scroll { direction: *direction, count: *count, by, bytes: bytes.len() })
            }
        }
    }

    fn running(&self) -> Result<std::sync::MutexGuard<'_, State>> {
        let state = self.state.lock().unwrap();
        match state.exit {
            None => Ok(state),
            Some(code) => Err(self.exited(code).into()),
        }
    }

    fn press(&self, key: Key) -> Result<usize> {
        let modes = self.state.lock().unwrap().screen.keyboard_modes();
        let bytes = keyboard::encode(key, modes).map_err(anyhow::Error::msg)?;
        self.write(&bytes)?;
        Ok(bytes.len())
    }

    fn pause(&self, delay: Duration) -> Result<()> {
        let until = Instant::now() + delay;
        let mut state = self.state.lock().unwrap();
        loop {
            if let Some(code) = state.exit {
                return Err(self.exited(code).into());
            }
            let now = Instant::now();
            if now >= until {
                return Ok(());
            }
            state = self.changed.wait_timeout(state, until - now).unwrap().0;
        }
    }

    fn write(&self, bytes: &[u8]) -> Result<()> {
        {
            let mut state = self.running()?;
            if let Some(commands) = &mut state.commands
                && bytes.iter().any(|byte| matches!(byte, b'\r' | b'\n'))
            {
                commands.entered();
            }
        }
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(bytes).and_then(|()| writer.flush()).with_context(|| format!("write to {}", self.program))?;
        self.recording.input(bytes);
        Ok(())
    }

    fn ensure_running(&self) -> Result<()> {
        self.running().map(drop)
    }

    fn exited(&self, code: Option<i32>) -> crate::Error {
        let how = code.map_or_else(String::new, |code| format!(" with code {code}"));
        crate::Error::new(
            crate::exit::EXITED,
            format!(
                "{} has exited{how}; nothing more can be sent to it (read its last screen with `agent-terminal snapshot`, run it again with `agent-terminal restart`)",
                self.program
            ),
        )
    }

    pub fn shell(&self) -> Option<Shell> {
        self.shell
    }

    pub fn submit(&self, line: &str) -> Result<Option<Submitted>> {
        let _turn = self.input_turn.lock().unwrap();
        let submitted = {
            let state = self.state.lock().unwrap();
            match &state.commands {
                Some(commands) if commands.prompt().ready => commands.submitted(),
                _ => return Ok(None),
            }
        };
        let mut bytes = line.as_bytes().to_vec();
        bytes.push(b'\r');
        self.write(&bytes)?;
        Ok(Some(submitted))
    }

    pub fn running_command(&self) -> Option<String> {
        self.state.lock().unwrap().commands.as_ref().and_then(|commands| commands.running().map(str::to_string))
    }

    pub fn last_exit_code(&self) -> Option<i32> {
        self.state.lock().unwrap().commands.as_ref().and_then(Commands::last_exit_code)
    }

    pub fn commands(&self) -> Option<Vec<CommandRecord>> {
        self.state.lock().unwrap().commands.as_ref().map(Commands::records)
    }

    pub fn command(&self, id: u32) -> Option<(CommandRecord, String)> {
        let state = self.state.lock().unwrap();
        let (record, from, to) = state.commands.as_ref()?.record(id)?;
        Some((record, state.log.text_between(from, to)))
    }

    pub fn styled_command(&self, id: u32) -> Option<(CommandRecord, String)> {
        let state = self.state.lock().unwrap();
        let (record, from, to) = state.commands.as_ref()?.record(id)?;
        Some((record, state.log.styled_between(from, to)))
    }

    pub fn exit_status(&self) -> Option<Option<i32>> {
        self.state.lock().unwrap().exit
    }

    pub fn lines(&self) -> Vec<String> {
        self.state.lock().unwrap().screen.view().lines
    }

    pub fn cell(&self, row: u16, col: u16) -> Result<Cell, String> {
        cell_at(&self.state.lock().unwrap().screen, row, col)
    }

    pub fn kill(&self) {
        let still_running = self.state.lock().unwrap().exit.is_none();
        if let Err(e) = self.killer.lock().unwrap().kill()
            && still_running
        {
            eprintln!("{}: stopping the program failed: {e}", self.program);
        }
    }
}

fn matches_on(screen: &Screen, pattern: &regex::Regex, first: i32) -> Vec<Match> {
    let mut matches = Vec::new();
    for line in first..screen.size().rows as i32 {
        let (row, columns) = screen.located_row(line);
        for found in pattern.find_iter(&row).filter(|m| !m.is_empty()) {
            matches.push(Match {
                row: if line < 0 { line } else { line + 1 },
                col: columns[found.start()] + 1,
                text: found.as_str().to_string(),
            });
        }
    }
    matches
}

fn no_mouse() -> anyhow::Error {
    anyhow::anyhow!(
        "this app has no mouse support; use keys instead (`agent-terminal snapshot -i` lists the key hints on screen)"
    )
}

fn click_target(state: &State, target: &ClickTarget) -> Result<(u16, u16)> {
    match target {
        ClickTarget::Cell { row, col } => {
            cell_at(&state.screen, *row, *col).map_err(anyhow::Error::msg)?;
            Ok((*row, *col))
        }
        ClickTarget::Text(text) => {
            let pattern = wait::compile(text, false).map_err(anyhow::Error::msg)?;
            let found = matches_on(&state.screen, &pattern, 0).into_iter().next();
            let found = found.ok_or_else(|| anyhow::anyhow!("{text:?} is not on the screen"))?;
            Ok((found.row as u16, found.col))
        }
        ClickTarget::Ref(id) => {
            let missing = || {
                anyhow::anyhow!(
                    "there is no @t{id} in the last `agent-terminal snapshot -i` of this tab; take one and use its refs"
                )
            };
            let listed = state.listed.as_ref().ok_or_else(missing)?;
            let found = (*id as usize).checked_sub(1).and_then(|index| listed.refs.get(index)).ok_or_else(missing)?;
            if !found.still_on(&state.screen.grid()) {
                anyhow::bail!(
                    "@t{id} is from frame {}, and that part of the screen has changed since; take a new `agent-terminal snapshot -i` and use its refs",
                    listed.frame
                );
            }
            Ok((found.row as u16 + 1, found.col as u16 + 1))
        }
    }
}

fn row_range(size: Size, rows: Option<RowRange>) -> Result<RowRange, String> {
    match rows {
        None => Ok(RowRange { first: 1, last: size.rows }),
        Some(RowRange { first, last }) if first == 0 || first > size.rows || first > last => {
            Err(format!("rows {first}:{last} are outside the {}x{} screen", size.cols, size.rows))
        }
        Some(RowRange { first, last }) => Ok(RowRange { first, last: last.min(size.rows) }),
    }
}

fn list_refs(state: &mut State) -> Vec<Ref> {
    let grid = state.screen.grid();
    remember_refs(state, &grid)
}

fn remember_refs(state: &mut State, grid: &Grid) -> Vec<Ref> {
    let found = refs::find(grid);
    let frame = state.frame;
    let listed = found.iter().enumerate().map(|(index, found)| listed_ref(index, found, frame)).collect();
    state.listed = Some(Listed { frame, refs: found });
    listed
}

fn listed_ref(index: usize, found: &Found, frame: u64) -> Ref {
    let place = |at: usize| at as u16 + 1;
    Ref {
        id: format!("@t{}", index + 1),
        kind: found.kind,
        label: found.label.clone(),
        row: place(found.row),
        col: place(found.col),
        rows: [place(found.area.row), place(found.area.last_row)],
        cols: [place(found.area.col), place(found.area.last_col)],
        frame,
        url: found.url.clone(),
    }
}

fn cell_at(screen: &Screen, row: u16, col: u16) -> Result<Cell, String> {
    screen.cell(row, col).ok_or_else(|| {
        let size = screen.view().size;
        format!("cell {row},{col} is outside the {}x{} screen", size.cols, size.rows)
    })
}

fn display_name(program: &str) -> String {
    std::path::Path::new(program).file_name().map_or_else(|| program.to_string(), |n| n.to_string_lossy().into_owned())
}
