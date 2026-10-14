use std::io::{Read, Write};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::native::pty::{self, Killer};
use crate::native::screen::{Screen, ScreenView};

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
    exit_code: Option<i32>,
}

pub struct Capture {
    pub view: ScreenView,
    pub frame: u64,
    pub exit_code: Option<i32>,
}

pub enum Settled {
    Stable,
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
            state: Mutex::new(State {
                screen: Screen::new(size),
                frame: 0,
                last_change: Instant::now(),
                exit_code: None,
            }),
            changed: Condvar::new(),
            writer: Mutex::new(spawned.writer),
            killer: Mutex::new(spawned.killer),
        });

        let reading = Arc::clone(&terminal);
        let mut reader = spawned.reader;
        let output_done = std::thread::spawn(move || {
            let mut buf = vec![0u8; 64 * 1024];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                reading.output(&buf[..n]);
            }
        });

        let waiting = Arc::clone(&terminal);
        let wait = spawned.wait;
        std::thread::spawn(move || {
            let code = wait();
            let _ = output_done.join();
            let mut state = waiting.state.lock().unwrap();
            state.exit_code = Some(code);
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
            let _ = writer.write_all(&replies);
            let _ = writer.flush();
        }
    }

    pub fn capture(&self) -> Capture {
        let state = self.state.lock().unwrap();
        Capture { view: state.screen.view(), frame: state.frame, exit_code: state.exit_code }
    }

    pub fn wait_stable(&self, quiet: Duration, timeout: Duration) -> Settled {
        let deadline = Instant::now() + timeout;
        let mut state = self.state.lock().unwrap();
        loop {
            if state.exit_code.is_some() {
                return Settled::Exited;
            }
            let now = Instant::now();
            let stable_at = (state.frame > 0).then(|| state.last_change + quiet);
            if stable_at.is_some_and(|at| now >= at) {
                return Settled::Stable;
            }
            if now >= deadline {
                return Settled::TimedOut;
            }
            let wake = stable_at.map_or(deadline, |at| at.min(deadline)) - now;
            state = self.changed.wait_timeout(state, wake).unwrap().0;
        }
    }

    pub fn kill(&self) {
        self.killer.lock().unwrap().kill();
    }
}

fn display_name(program: &str) -> String {
    std::path::Path::new(program).file_name().map_or_else(|| program.to_string(), |n| n.to_string_lossy().into_owned())
}
