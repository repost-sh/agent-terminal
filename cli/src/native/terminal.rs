use std::io::{Read, Write};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::native::pty::{self, Killer};
use crate::native::screen::Screen;
use crate::protocol::{Cursor, Exit, Fact, Get, Match, RowRange, Scrollback, Snapshot, SnapshotOptions, Wait};

pub struct Terminal {
    pub program: String,
    pid: Option<u32>,
    cwd: String,
    state: Mutex<State>,
    changed: Condvar,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn Killer>>,
}

struct State {
    screen: Screen,
    frame: u64,
    last_change: Instant,
    exit: Option<Option<i32>>,
}

pub enum Settled {
    Met,
    Exited,
    TimedOut,
}

impl Terminal {
    pub fn start(command: pty::Command) -> Result<Arc<Self>> {
        let program = display_name(&command.program);
        let size = command.size;
        let cwd = command.cwd.to_string_lossy().into_owned();
        let spawned = pty::spawn(command)?;
        let terminal = Arc::new(Self {
            program,
            pid: spawned.pid,
            cwd,
            state: Mutex::new(State { screen: Screen::new(size), frame: 0, last_change: Instant::now(), exit: None }),
            changed: Condvar::new(),
            writer: Mutex::new(spawned.writer),
            killer: Mutex::new(spawned.killer),
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
        });

        Ok(terminal)
    }

    fn output(&self, bytes: &[u8]) {
        let replies = {
            let mut state = self.state.lock().unwrap();
            let replies = state.screen.feed(bytes);
            state.frame += 1;
            state.last_change = Instant::now();
            replies
        };
        self.changed.notify_all();
        if !replies.is_empty() {
            let mut writer = self.writer.lock().unwrap();
            if let Err(e) = writer.write_all(&replies).and_then(|()| writer.flush()) {
                eprintln!("{}: answering a terminal query failed: {e}", self.program);
            }
        }
    }

    pub fn snapshot(&self, session: &str, tab: &str) -> Snapshot {
        let state = self.state.lock().unwrap();
        let rows = state.screen.view().size.rows;
        self.render(&state, session, tab, RowRange { first: 1, last: rows }, &SnapshotOptions::default())
    }

    pub fn snapshot_with(&self, session: &str, tab: &str, options: &SnapshotOptions) -> Result<Snapshot, String> {
        let state = self.state.lock().unwrap();
        let size = state.screen.view().size;
        let range = match options.rows {
            None => RowRange { first: 1, last: size.rows },
            Some(RowRange { first, last }) if first == 0 || first > size.rows || first > last => {
                return Err(format!("rows {first}:{last} are outside the {}x{} screen", size.cols, size.rows));
            }
            Some(RowRange { first, last }) => RowRange { first, last: last.min(size.rows) },
        };
        Ok(self.render(&state, session, tab, range, options))
    }

    fn render(&self, state: &State, session: &str, tab: &str, range: RowRange, options: &SnapshotOptions) -> Snapshot {
        let view = state.screen.view();
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
