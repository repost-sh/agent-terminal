use std::time::{Duration, Instant};

use super::expect::exited_otherwise;
use super::refusals::{error_body, human_has_control, no_integration};
use super::{Daemon, Tab};
use crate::native::commands::Submitted;
use crate::native::control::{Control, Handed};
use crate::native::terminal::{Labelled, Settled, Status, Terminal};
use crate::native::wait::Condition;
use crate::protocol::{CommandOutput, ErrorBody, Reply, RowRange, Snapshot, TextMatch, Wait, WaitOutcome, request};

const CONTROL_HANDED_BACK: &str = "a human to hand back control";

fn longest(wait: &Wait, timeout_ms: u64) -> u64 {
    match wait {
        Wait::Elapsed { ms } => timeout_ms.max(*ms),
        _ => timeout_ms,
    }
}

struct Missed<'a> {
    assert: bool,
    wait: &'a Wait,
    timeout_ms: u64,
    session: &'a str,
    tab: &'a str,
}

fn failed(error: ErrorBody, missed: &Missed<'_>) -> ErrorBody {
    match error {
        ErrorBody { code: crate::exit::TIMEOUT, snapshot, .. } if missed.assert => ErrorBody {
            snapshot,
            ..ErrorBody::new(
                crate::exit::ASSERTION,
                format!(
                    "{} within {} ms in {} {}",
                    not_met(missed.wait),
                    missed.timeout_ms,
                    missed.session,
                    missed.tab
                ),
            )
        },
        error => error,
    }
}

fn exit_code_after(wait: &Wait, terminal: &Terminal) -> Option<i32> {
    match wait {
        Wait::Exit { .. } => terminal.status().code(),
        Wait::Command => terminal.last_exit_code(),
        _ => None,
    }
}

impl Daemon {
    pub(super) fn wait(&self, waiting: request::WaitFor) -> Result<Reply, ErrorBody> {
        let request::WaitFor { target, wait, timeout_ms, assert } = waiting;
        let timeout_ms = longest(&wait, timeout_ms);
        let (tab, terminal) = self.target(&target)?;
        let (session, tab) = (String::from(target.session), tab.to_string());
        let at = Labelled { session: &session, tab: &tab, terminal: &terminal };
        if matches!(wait, Wait::Command) && terminal.shell().is_none() {
            return Err(no_integration(&at));
        }
        let condition = condition(&wait)?;
        let started = Instant::now();
        let missed = Missed { assert, wait: &wait, timeout_ms, session: &session, tab: &tab };
        let frame =
            Until { wait: &wait, timeout_ms }.wait_on(&at, condition).map_err(|error| failed(error, &missed))?;
        let waited_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let exit_code = exit_code_after(&wait, &terminal);
        Ok(Reply::Waited(WaitOutcome { session, tab, until: describe(&wait), waited_ms, frame, exit_code }))
    }

    pub(super) fn wait_control(&self, waiting: request::WaitControl) -> Result<Reply, ErrorBody> {
        let request::WaitControl { target, timeout_ms, assert } = waiting;
        let (id, Tab { name, terminal, control, .. }) = self.tab(&target)?;
        let (session, tab) = (String::from(target.session), name.to_string());
        let started = Instant::now();
        match control.wait_for_agent(Duration::from_millis(timeout_ms)) {
            Handed::Back => {
                let frame = self.terminal_of(&session, id).unwrap_or(terminal).frame();
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
        let (_, Tab { name, terminal, control, .. }) = self.tab(&exec.target)?;
        let (session, tab) = (String::from(exec.target.session), name.to_string());
        let at = Labelled { session: &session, tab: &tab, terminal: &terminal };
        if terminal.shell().is_none() {
            return Err(no_integration(&at));
        }
        let running = Running {
            at,
            line: &exec.line,
            timeout_ms: exec.timeout_ms,
            deadline: Instant::now() + Duration::from_millis(exec.timeout_ms),
        };
        let submitted = running.submit(&control)?;
        running.finish(&submitted)?;
        let (record, output) = terminal.command(submitted.first_id).ok_or_else(|| {
            let message =
                format!("the shell in {session} {tab} ran no command for `{}`; the screen shows why", exec.line);
            ErrorBody::new(crate::exit::ERROR, message).with_screen(running.snapshot())
        })?;
        Ok(Reply::Command(CommandOutput { session, tab, record, output, check: exec.check }))
    }
}

struct Running<'a> {
    at: Labelled<'a>,
    line: &'a str,
    timeout_ms: u64,
    deadline: Instant,
}

impl Running<'_> {
    fn left(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    fn snapshot(&self) -> Snapshot {
        self.at.snapshot()
    }

    fn submit(&self, control: &Control) -> Result<Submitted, ErrorBody> {
        loop {
            let submitted = {
                let _turn = control.agent().ok_or_else(|| human_has_control(self.at.session, self.at.tab))?;
                self.at
                    .terminal
                    .submit(self.line)
                    .map_err(|e| error_body(&e.context(format!("{} {}", self.at.session, self.at.tab))))?
            };
            if let Some(submitted) = submitted {
                return Ok(submitted);
            }
            match self.at.terminal.wait(Condition::Command, self.left()) {
                Settled::Met { .. } => {}
                Settled::Exited => return Err(exited_while_waiting(&Wait::Command, self.snapshot())),
                Settled::TimedOut => return Err(self.shell_busy()),
            }
        }
    }

    fn shell_busy(&self) -> ErrorBody {
        let waiting = match self.at.terminal.running_command() {
            Some(running) => format!("`{running}` to finish"),
            None => "the shell prompt".to_string(),
        };
        let message = format!(
            "timed out after {} ms waiting for {waiting} in {} {}, so `{}` did not run (read the screen with `agent-terminal snapshot`, interrupt a command with `agent-terminal signal INT`)",
            self.timeout_ms, self.at.session, self.at.tab, self.line
        );
        ErrorBody::new(crate::exit::TIMEOUT, message).with_screen(self.snapshot())
    }

    fn finish(&self, submitted: &Submitted) -> Result<(), ErrorBody> {
        match self.at.terminal.wait(Condition::PromptAfter(submitted.prompts), self.left()) {
            Settled::Met { .. } => Ok(()),
            Settled::Exited => Err(exited_while_waiting(&Wait::Command, self.snapshot())),
            Settled::TimedOut => Err(self.still_running(submitted.first_id)),
        }
    }

    fn still_running(&self, id: u32) -> ErrorBody {
        let waited = format!(
            "timed out after {} ms waiting for `{}` to finish in {} {}",
            self.timeout_ms, self.line, self.at.session, self.at.tab
        );
        let message = match self.at.terminal.command(id) {
            Some(_) => format!(
                "{waited}; it is still running as command {id} (wait for it with `agent-terminal wait --command`, read its output with `agent-terminal history show {id}`, interrupt it with `agent-terminal signal INT`)"
            ),
            None => format!(
                "{waited}; the shell has not started a command for it, so the line may be unfinished (read the screen with `agent-terminal snapshot`, cancel the line with `agent-terminal press Ctrl+c`)"
            ),
        };
        ErrorBody::new(crate::exit::TIMEOUT, message).with_screen(self.snapshot())
    }
}

pub(super) fn condition(wait: &Wait) -> Result<Condition, ErrorBody> {
    Condition::new(wait).map_err(ErrorBody::failure)
}

pub(super) struct Until<'a> {
    pub wait: &'a Wait,
    pub timeout_ms: u64,
}

impl Until<'_> {
    pub(super) fn wait_on(&self, at: &Labelled<'_>, condition: Condition) -> Result<u64, ErrorBody> {
        match at.terminal.wait(condition, Duration::from_millis(self.timeout_ms)) {
            Settled::Met { frame } => match (self.wait, at.terminal.status()) {
                (Wait::Exit { code: Some(expected) }, Status::Exited(code)) if code != Some(*expected) => {
                    Err(exited_otherwise(*expected, at.snapshot()))
                }
                _ => Ok(frame),
            },
            Settled::Exited => Err(exited_while_waiting(self.wait, at.snapshot())),
            Settled::TimedOut => Err(self.timed_out(at.snapshot())),
        }
    }

    pub(super) fn timed_out(&self, snapshot: Snapshot) -> ErrorBody {
        let message = format!(
            "timed out after {} ms waiting for {}; {} {} is still running (read it with `agent-terminal snapshot`, stop it with `agent-terminal close`)",
            self.timeout_ms,
            describe(self.wait),
            snapshot.session,
            snapshot.tab
        );
        ErrorBody::new(crate::exit::TIMEOUT, message).with_screen(snapshot)
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
    ErrorBody::new(crate::exit::EXITED, message).with_screen(snapshot)
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

#[cfg(test)]
mod tests;
