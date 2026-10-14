use std::io::{Read, Write};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::key::Key;
use crate::native::keyboard;
use crate::native::pty::{self, Killer, Signaller};
use crate::native::screen::Screen;
use crate::protocol::{Delivery, Input, Size, Snapshot, TabInfo, Wait};

pub struct Terminal {
    pub program: String,
    state: Mutex<State>,
    changed: Condvar,
    input_turn: Mutex<()>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn Killer>>,
    master: pty::Master,
    signaller: Box<dyn Signaller>,
    command: pty::Command,
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
        let spawned = pty::spawn(command.clone())?;
        let terminal = Arc::new(Self {
            program,
            state: Mutex::new(State { screen: Screen::new(size), frame: 0, last_change: Instant::now(), exit: None }),
            changed: Condvar::new(),
            input_turn: Mutex::new(()),
            writer: Mutex::new(spawned.writer),
            killer: Mutex::new(spawned.killer),
            master: spawned.master,
            signaller: spawned.signaller,
            command,
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
        Ok(())
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
