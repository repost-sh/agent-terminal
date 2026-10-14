use std::io::{Read, Write};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::native::pty::{self, Killer};
use crate::native::screen::Screen;
use crate::protocol::{Snapshot, Wait};

pub struct Terminal {
    pub program: String,
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
        let spawned = pty::spawn(command)?;
        let terminal = Arc::new(Self {
            program,
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
        let view = state.screen.view();
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
            lines: view.lines,
        }
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
