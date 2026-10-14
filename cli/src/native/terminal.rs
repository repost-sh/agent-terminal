use std::io::{Read, Write};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tokio::sync::watch;

use crate::key::Key;
use crate::native::changes::Changes;
use crate::native::keyboard;
use crate::native::output_log::OutputLog;
use crate::native::pty::{self, Killer, Signaller};
use crate::native::screen::Screen;
use crate::protocol::{
    Baseline, Cursor, Delivery, DiffBase, Exit, Fact, Get, Input, Match, RowChange, RowRange, Scrollback, Size,
    Snapshot, SnapshotDiff, SnapshotOptions, TabInfo, Wait,
};

pub struct Terminal {
    pub program: String,
    pid: Option<u32>,
    cwd: String,
    state: Mutex<State>,
    changed: Condvar,
    input_turn: Mutex<()>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn Killer>>,
    master: pty::Master,
    signaller: Box<dyn Signaller>,
    command: pty::Command,
    frames: Changes,
    lifecycle: Changes,
}

pub struct Frame {
    pub number: u64,
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
    last_change: Instant,
    exit: Option<Option<i32>>,
    log: OutputLog,
    last_shown: Option<Shown>,
}

struct Shown {
    frame: u64,
    lines: Vec<String>,
}

pub enum Settled {
    Met,
    Exited,
    TimedOut,
}

impl Terminal {
    pub fn start(command: pty::Command, lifecycle: Changes) -> Result<Arc<Self>> {
        let program = display_name(&command.program);
        let size = command.size;
        let cwd = command.cwd.to_string_lossy().into_owned();
        let spawned = pty::spawn(command.clone())?;
        let terminal = Arc::new(Self {
            program,
            pid: spawned.pid,
            cwd,
            state: Mutex::new(State {
                screen: Screen::new(size),
                frame: 0,
                last_change: Instant::now(),
                exit: None,
                log: OutputLog::default(),
                last_shown: None,
            }),
            changed: Condvar::new(),
            input_turn: Mutex::new(()),
            writer: Mutex::new(spawned.writer),
            killer: Mutex::new(spawned.killer),
            master: spawned.master,
            signaller: spawned.signaller,
            command,
            frames: Changes::default(),
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

        Ok(terminal)
    }

    fn output(&self, bytes: &[u8]) {
        let replies = {
            let mut state = self.state.lock().unwrap();
            let replies = state.screen.feed(bytes);
            let printed = state.screen.take_printed();
            let frame = state.frame;
            state.log.record(frame, bytes, printed.as_bytes());
            state.frame += 1;
            state.last_change = Instant::now();
            replies
        };
        self.changed.notify_all();
        self.frames.notify();
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
        let size = state.screen.view().size;
        let range = match options.rows {
            None => RowRange { first: 1, last: size.rows },
            Some(RowRange { first, last }) if first == 0 || first > size.rows || first > last => {
                return Err(format!("rows {first}:{last} are outside the {}x{} screen", size.cols, size.rows));
            }
            Some(RowRange { first, last }) => RowRange { first, last: last.min(size.rows) },
        };
        Ok(self.render(&mut state, session, tab, range, options))
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
        state.frame += 1;
        state.last_change = Instant::now();
        drop(state);
        self.changed.notify_all();
        self.frames.notify();
        self.lifecycle.notify();
        Ok(())
    }

    pub fn frames(&self) -> watch::Receiver<u64> {
        self.frames.subscribe()
    }

    pub fn frame(&self) -> Frame {
        let state = self.state.lock().unwrap();
        let view = state.screen.view();
        Frame {
            number: state.frame,
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
            Get::Cell { row, col } => Fact::Cell(state.screen.cell(row, col).ok_or_else(|| {
                format!("cell {row},{col} is outside the {}x{} screen", view.size.cols, view.size.rows)
            })?),
        })
    }

    pub fn find(&self, text: &str, regex: bool, all: bool) -> Result<Vec<Match>, String> {
        let source = if regex { text.to_string() } else { regex::escape(text) };
        let pattern = regex::Regex::new(&source).map_err(|e| format!("invalid regex {text:?}: {e}"))?;
        let state = self.state.lock().unwrap();
        let first = if all { -(state.screen.scrollback_len() as i32) } else { 0 };
        let mut matches = Vec::new();
        for line in first..state.screen.view().size.rows as i32 {
            let (row, columns) = state.screen.located_row(line);
            for found in pattern.find_iter(&row).filter(|m| !m.is_empty()) {
                matches.push(Match {
                    row: if line < 0 { line } else { line + 1 },
                    col: columns[found.start()] + 1,
                    text: found.as_str().to_string(),
                });
            }
        }
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

    pub fn printed(&self, since: Option<&str>, raw: bool) -> Result<String, String> {
        let state = self.state.lock().unwrap();
        state.log.since(since, raw, state.frame)
    }

    pub fn wait(&self, until: &Wait, timeout: Duration) -> Settled {
        let deadline = Instant::now() + timeout;
        let mut state = self.state.lock().unwrap();
        loop {
            let now = Instant::now();
            let met_at = match until {
                Wait::Stable { quiet_ms } => Some(state.last_change + Duration::from_millis(*quiet_ms)),
                Wait::Text { text } => state.screen.view().lines.iter().any(|line| line.contains(text)).then_some(now),
            };
            if met_at.is_some_and(|at| now >= at) {
                return Settled::Met;
            }
            if state.exit.is_some() {
                return Settled::Exited;
            }
            if now >= deadline {
                return Settled::TimedOut;
            }
            let wake = met_at.map_or(deadline, |at| at.min(deadline)) - now;
            state = self.changed.wait_timeout(state, wake).unwrap().0;
        }
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
        self.ensure_running()?;
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(bytes).and_then(|()| writer.flush()).with_context(|| format!("write to {}", self.program))
    }

    fn ensure_running(&self) -> Result<()> {
        match self.state.lock().unwrap().exit {
            None => Ok(()),
            Some(code) => Err(self.exited(code).into()),
        }
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

    pub fn kill(&self) {
        let still_running = self.state.lock().unwrap().exit.is_none();
        if let Err(e) = self.killer.lock().unwrap().kill()
            && still_running
        {
            eprintln!("{}: stopping the program failed: {e}", self.program);
        }
    }
}

fn display_name(program: &str) -> String {
    std::path::Path::new(program).file_name().map_or_else(|| program.to_string(), |n| n.to_string_lossy().into_owned())
}
