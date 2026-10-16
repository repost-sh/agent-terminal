mod picture;
mod program_output;
mod reading;
mod shell_history;
mod state;
mod status;
mod typing;
mod waiting;

use std::io::Write;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use anyhow::Result;
use tokio::sync::watch;

use crate::failure::Failure;
use crate::native::changes::Changes;
use crate::native::commands::Commands;
use crate::native::log;
use crate::native::pty::{self, Signaller, Spawned, Stopper};
use crate::native::screen::Screen;
use crate::native::store::CommandLog;
use crate::protocol::{Modes, Name, Owner, Shell, Size, TabInfo};
pub(crate) use picture::{Glance, Picture};
use state::State;
pub(crate) use status::Status;
pub(crate) use waiting::Settled;

pub(crate) const HANG_UP_GRACE: Duration = Duration::from_secs(1);

pub(crate) struct Terminal {
    program: String,
    pid: Option<u32>,
    cwd: String,
    started: SystemTime,
    state: Mutex<State>,
    changed: Condvar,
    input_turn: Mutex<()>,
    writer: Mutex<Box<dyn Write + Send>>,
    stopper: Stopper,
    master: pty::Master,
    signaller: Box<dyn Signaller>,
    command: pty::Command,
    shell: Option<Shell>,
    command_log: CommandLog,
    frames: Changes,
    history: Changes,
    lifecycle: Changes,
}

pub(crate) struct Setup {
    pub command: pty::Command,
    pub pid: Option<u32>,
    pub cwd: String,
    pub size: Size,
    pub modes: Modes,
    pub started: SystemTime,
    pub status: Status,
}

pub(crate) struct Continued {
    frame: u64,
    first_command: u32,
}

pub(crate) struct Starting {
    pub command: pty::Command,
    pub shell: Option<Shell>,
    pub spawned: Spawned,
    pub command_log: CommandLog,
    pub lifecycle: Changes,
    pub continued: Option<Continued>,
}

#[derive(Clone, Copy)]
pub(crate) struct Labelled<'a> {
    pub session: &'a str,
    pub tab: &'a str,
    pub terminal: &'a Terminal,
}

impl Terminal {
    pub(crate) fn start(starting: Starting) -> Arc<Self> {
        let Starting { command, shell, spawned, command_log, lifecycle, continued } = starting;
        let cwd = command.cwd.to_string_lossy().into_owned();
        let Continued { frame, first_command } = continued.unwrap_or(Continued { frame: 0, first_command: 1 });
        let commands = shell.map(|_| Commands::new(cwd.clone(), first_command));
        let terminal = Arc::new(Self {
            program: display_name(&command.program),
            pid: spawned.pid,
            cwd,
            started: SystemTime::now(),
            state: Mutex::new(State { frame, commands, ..State::new(Screen::new(command.size)) }),
            changed: Condvar::new(),
            input_turn: Mutex::new(()),
            writer: Mutex::new(spawned.writer),
            stopper: spawned.stopper,
            master: spawned.master,
            signaller: spawned.signaller,
            command,
            shell,
            command_log,
            frames: Changes::default(),
            history: Changes::default(),
            lifecycle,
        });
        terminal.follow_output(spawned.reader, spawned.wait);
        terminal
    }

    pub(crate) fn program(&self) -> &str {
        &self.program
    }

    pub(crate) fn resize(&self, size: Size) -> Result<()> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Status::Exited(code) = state.status {
            return Err(self.exited(code).into());
        }
        if !self.master.resize(size)? {
            return Err(self.exited(None).into());
        }
        state.screen.resize(size);
        state.refresh(Instant::now());
        drop(state);
        self.changed.notify_all();
        self.frames.notify();
        self.lifecycle.notify();
        Ok(())
    }

    pub(crate) fn frame(&self) -> u64 {
        self.state.lock().unwrap_or_else(PoisonError::into_inner).frame
    }

    pub(crate) fn frames(&self) -> watch::Receiver<u64> {
        self.frames.subscribe()
    }

    pub(crate) fn picture(&self) -> Picture {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        Picture::of(&state)
    }

    pub(crate) fn glance(&self) -> Glance {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        Glance::of(&state)
    }

    pub(crate) fn continued(&self) -> Continued {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        Continued { frame: state.frame, first_command: state.commands.as_ref().map_or(1, Commands::next_id) }
    }

    pub(crate) fn setup(&self) -> Setup {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        Setup {
            command: self.command.clone(),
            pid: self.pid,
            cwd: state.screen.reported_cwd().unwrap_or_else(|| self.cwd.clone()),
            size: state.screen.size(),
            modes: state.screen.modes(),
            started: self.started,
            status: state.status,
        }
    }

    pub(crate) fn command_log(&self) -> CommandLog {
        self.command_log.clone()
    }

    pub(crate) fn history_changes(&self) -> watch::Receiver<u64> {
        self.history.subscribe()
    }

    pub(crate) fn rerun(&self) -> pty::Command {
        let size = self.state.lock().unwrap_or_else(PoisonError::into_inner).screen.size();
        pty::Command { size, ..self.command.clone() }
    }

    pub(crate) fn stop(&self, timeout: Duration) -> bool {
        self.hang_up();
        let deadline = Instant::now() + timeout;
        self.exited_within(timeout.min(HANG_UP_GRACE)) || {
            self.kill();
            self.exited_within(deadline.saturating_duration_since(Instant::now()))
        }
    }

    pub(crate) fn hang_up(&self) {
        if self.status() == Status::Running
            && let Err(e) = self.stopper.hang_up()
        {
            log::note(format_args!("{}: hanging up failed: {e:#}", self.program));
        }
    }

    pub(crate) fn kill(&self) {
        if self.status() == Status::Running
            && let Err(e) = self.stopper.kill()
        {
            log::note(format_args!("{}: killing the program failed: {e:#}", self.program));
        }
    }

    pub(crate) fn exited_within(&self, timeout: Duration) -> bool {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let (state, _) = self
            .changed
            .wait_timeout_while(state, timeout, |state| state.status == Status::Running)
            .unwrap_or_else(PoisonError::into_inner);
        state.status.exited()
    }

    pub(crate) fn status(&self) -> Status {
        self.state.lock().unwrap_or_else(PoisonError::into_inner).status
    }

    pub(crate) fn info(&self, tab: &Name, control: Owner) -> TabInfo {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let size = state.screen.size();
        TabInfo {
            tab: tab.clone(),
            renamed_from: None,
            program: self.program.clone(),
            command: match self.shell {
                Some(_) => crate::output::call_line(std::slice::from_ref(&self.command.program)),
                None => self.command.line(),
            },
            cwd: self.cwd.clone(),
            cols: size.cols,
            rows: size.rows,
            exited: state.status.exited(),
            exit_code: state.status.code(),
            control,
        }
    }

    fn exited(&self, code: Option<i32>) -> Failure {
        let how = code.map_or_else(String::new, |code| format!(" with code {code}"));
        Failure::new(
            crate::exit::EXITED,
            format!(
                "{} has exited{how}; nothing more can be sent to it (read its last screen with `agent-terminal snapshot`, run it again with `agent-terminal restart`)",
                self.program
            ),
        )
    }
}

fn display_name(program: &str) -> String {
    std::path::Path::new(program).file_name().map_or_else(|| program.to_string(), |n| n.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests;
