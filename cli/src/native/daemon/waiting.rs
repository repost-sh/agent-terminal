use std::time::{Duration, Instant};

use super::expect::exited_otherwise;
use super::refusals::{error_body, human_has_control, no_integration};
use super::{Daemon, Tab};
use crate::native::commands::Submitted;
use crate::native::control::{Control, Handed};
use crate::native::terminal::{Settled, Status, Terminal};
use crate::native::wait::Condition;
use crate::protocol::{
    CommandOutput, ErrorBody, Reply, RowRange, Snapshot, Target, TextMatch, Wait, WaitOutcome, request,
};

const CONTROL_HANDED_BACK: &str = "a human to hand back control";

impl Daemon {
    pub(super) fn wait(&self, waiting: request::WaitFor) -> Result<Reply, ErrorBody> {
        let request::WaitFor { target, wait, timeout_ms, assert } = waiting;
        let timeout_ms = match wait {
            Wait::Elapsed { ms } => timeout_ms.max(ms),
            _ => timeout_ms,
        };
        let (id, terminal) = self.target(&target)?;
        let (session, tab) = (target.session, id.to_string());
        if matches!(wait, Wait::Command) && terminal.shell().is_none() {
            return Err(no_integration(&session, &tab, &terminal));
        }
        let condition = condition(&wait)?;
        let started = Instant::now();
        let frame = wait_for(&terminal, &session, &tab, &wait, condition, timeout_ms).map_err(|error| match error {
            ErrorBody { code: crate::exit::TIMEOUT, snapshot, .. } if assert => ErrorBody {
                snapshot,
                ..ErrorBody::new(
                    crate::exit::ASSERTION,
                    format!("{} within {timeout_ms} ms in {session} {tab}", not_met(&wait)),
                )
            },
            error => error,
        })?;
        let waited_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let exit_code = match wait {
            Wait::Exit { .. } => terminal.status().code(),
            Wait::Command => terminal.last_exit_code(),
            _ => None,
        };
        Ok(Reply::Waited(WaitOutcome { session, tab, until: describe(&wait), waited_ms, frame, exit_code }))
    }

    pub(super) fn wait_control(&self, waiting: request::WaitControl) -> Result<Reply, ErrorBody> {
        let request::WaitControl { target, timeout_ms, assert } = waiting;
        let (id, Tab { terminal, control }) = self.tab(&target)?;
        let (session, tab) = (target.session, id.to_string());
        let started = Instant::now();
        match control.wait_for_agent(Duration::from_millis(timeout_ms)) {
            Handed::Back => {
                let frame =
                    self.target(&Target::tab(&session, id)).map_or_else(|_| terminal.frame(), |(_, now)| now.frame());
                let waited_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                let until = CONTROL_HANDED_BACK.to_string();
                Ok(Reply::Waited(WaitOutcome { session, tab, until, waited_ms, frame, exit_code: None }))
            }
            Handed::TimedOut if assert => Err(ErrorBody::new(
                crate::exit::ASSERTION,
                format!("a human did not hand back control of {session} {tab} within {timeout_ms} ms"),
            )),
            Handed::TimedOut => Err(ErrorBody::new(
                crate::exit::TIMEOUT,
                format!(
                    "timed out after {timeout_ms} ms waiting for {CONTROL_HANDED_BACK} of {session} {tab}; wait longer with --timeout <ms>"
                ),
            )),
            Handed::Closed => {
                Err(ErrorBody::failure(format!("stopped waiting for control: {session} {tab} was closed")))
            }
        }
    }

    pub(super) fn exec(&self, exec: request::Exec) -> Result<Reply, ErrorBody> {
        let (id, Tab { terminal, control }) = self.tab(&exec.target)?;
        let (session, tab) = (exec.target.session, id.to_string());
        if terminal.shell().is_none() {
            return Err(no_integration(&session, &tab, &terminal));
        }
        let running = Running {
            terminal: &terminal,
            session: &session,
            tab: &tab,
            line: &exec.line,
            timeout_ms: exec.timeout_ms,
            deadline: Instant::now() + Duration::from_millis(exec.timeout_ms),
        };
        let submitted = running.submit(&control)?;
        running.finish(&submitted)?;
        let (record, output) = terminal.command(submitted.first_id).ok_or_else(|| {
            let message =
                format!("the shell in {session} {tab} ran no command for `{}`; the screen shows why", exec.line);
            ErrorBody::with_screen(crate::exit::ERROR, message, terminal.snapshot(&session, &tab))
        })?;
        Ok(Reply::Command(CommandOutput { session, tab, record, output, check: exec.check }))
    }
}

struct Running<'a> {
    terminal: &'a Terminal,
    session: &'a str,
    tab: &'a str,
    line: &'a str,
    timeout_ms: u64,
    deadline: Instant,
}

impl Running<'_> {
    fn left(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    fn snapshot(&self) -> Snapshot {
        self.terminal.snapshot(self.session, self.tab)
    }

    fn submit(&self, control: &Control) -> Result<Submitted, ErrorBody> {
        loop {
            let submitted = {
                let _turn = control.agent().ok_or_else(|| human_has_control(self.session, self.tab))?;
                self.terminal
                    .submit(self.line)
                    .map_err(|e| error_body(&e.context(format!("{} {}", self.session, self.tab))))?
            };
            if let Some(submitted) = submitted {
                return Ok(submitted);
            }
            match self.terminal.wait(Condition::Command, self.left()) {
                Settled::Met { .. } => {}
                Settled::Exited => return Err(exited_while_waiting(&Wait::Command, self.snapshot())),
                Settled::TimedOut => return Err(self.shell_busy()),
            }
        }
    }

    fn shell_busy(&self) -> ErrorBody {
        let waiting = match self.terminal.running_command() {
            Some(running) => format!("`{running}` to finish"),
            None => "the shell prompt".to_string(),
        };
        let message = format!(
            "timed out after {} ms waiting for {waiting} in {} {}, so `{}` did not run (read the screen with `agent-terminal snapshot`, interrupt a command with `agent-terminal signal INT`)",
            self.timeout_ms, self.session, self.tab, self.line
        );
        ErrorBody::with_screen(crate::exit::TIMEOUT, message, self.snapshot())
    }

    fn finish(&self, submitted: &Submitted) -> Result<(), ErrorBody> {
        match self.terminal.wait(Condition::PromptAfter(submitted.prompts), self.left()) {
            Settled::Met { .. } => Ok(()),
            Settled::Exited => Err(exited_while_waiting(&Wait::Command, self.snapshot())),
            Settled::TimedOut => Err(self.still_running(submitted.first_id)),
        }
    }

    fn still_running(&self, id: u32) -> ErrorBody {
        let waited = format!(
            "timed out after {} ms waiting for `{}` to finish in {} {}",
            self.timeout_ms, self.line, self.session, self.tab
        );
        let message = match self.terminal.command(id) {
            Some(_) => format!(
                "{waited}; it is still running as command {id} (wait for it with `agent-terminal wait --command`, read its output with `agent-terminal history show {id}`, interrupt it with `agent-terminal signal INT`)"
            ),
            None => format!(
                "{waited}; the shell has not started a command for it, so the line may be unfinished (read the screen with `agent-terminal snapshot`, cancel the line with `agent-terminal press Ctrl+c`)"
            ),
        };
        ErrorBody::with_screen(crate::exit::TIMEOUT, message, self.snapshot())
    }
}

pub(super) fn condition(wait: &Wait) -> Result<Condition, ErrorBody> {
    Condition::new(wait).map_err(ErrorBody::failure)
}

pub(super) fn wait_for(
    terminal: &Terminal,
    session: &str,
    tab: &str,
    wait: &Wait,
    condition: Condition,
    timeout_ms: u64,
) -> Result<u64, ErrorBody> {
    match terminal.wait(condition, Duration::from_millis(timeout_ms)) {
        Settled::Met { frame } => match (wait, terminal.status()) {
            (Wait::Exit { code: Some(expected) }, Status::Exited(code)) if code != Some(*expected) => {
                Err(exited_otherwise(*expected, terminal.snapshot(session, tab)))
            }
            _ => Ok(frame),
        },
        Settled::Exited => Err(exited_while_waiting(wait, terminal.snapshot(session, tab))),
        Settled::TimedOut => Err(timed_out(wait, timeout_ms, terminal.snapshot(session, tab))),
    }
}

fn exited_while_waiting(wait: &Wait, snapshot: Snapshot) -> ErrorBody {
    let how = snapshot.exit_code.map_or_else(String::new, |code| format!(" with code {code}"));
    let message = format!(
        "stopped waiting for {}: {} in {} {} has exited{how}",
        describe(wait),
        snapshot.program,
        snapshot.session,
        snapshot.tab
    );
    ErrorBody::with_screen(crate::exit::EXITED, message, snapshot)
}

pub(super) fn timed_out(wait: &Wait, timeout_ms: u64, snapshot: Snapshot) -> ErrorBody {
    let message = format!(
        "timed out after {timeout_ms} ms waiting for {}; {} {} is still running (read it with `agent-terminal snapshot`, stop it with `agent-terminal close`)",
        describe(wait),
        snapshot.session,
        snapshot.tab
    );
    ErrorBody::with_screen(crate::exit::TIMEOUT, message, snapshot)
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
