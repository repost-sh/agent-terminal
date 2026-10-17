use std::cell::OnceCell;
use std::time::{Duration, Instant};

use regex::Regex;

use crate::native::commands::PromptState;
use crate::native::screen::{Screen, ScreenView};
use crate::protocol::{RowRange, TextMatch, Wait};

const FIRST_OUTPUT_GRACE: Duration = Duration::from_secs(2);

pub(crate) enum Condition {
    Text(Pattern),
    Gone(Pattern),
    Stable(Duration, Option<Instant>),
    FirstStable { quiet: Duration, silent_by: Instant },
    Idle(Duration, Option<Instant>),
    Exit,
    Cursor(u16, u16),
    Frame(u64),
    Elapsed(Instant),
    Command,
    PromptAfter(u64),
}

pub(crate) struct Pattern {
    regex: Regex,
    rows: Option<RowRange>,
}

pub(crate) fn compile(text: &str, regex: bool) -> Result<Regex, String> {
    let source = if regex { text.to_string() } else { regex::escape(text) };
    Regex::new(&source).map_err(|e| format!("invalid regex {text:?}: {e}"))
}

impl Pattern {
    pub(crate) fn new(text: &TextMatch) -> Result<Self, String> {
        Ok(Self { regex: compile(&text.text, text.regex)?, rows: text.rows })
    }

    pub(crate) fn rows_found(&self, lines: &[String]) -> Vec<u16> {
        let (first, last) = self.rows.map_or((1, u16::MAX), |rows| (rows.first, rows.last));
        (1..=u16::MAX)
            .zip(lines)
            .filter(|(row, line)| (first..=last).contains(row) && self.regex.is_match(line))
            .map(|(row, _)| row)
            .collect()
    }

    pub(crate) fn found(&self, lines: &[String]) -> bool {
        !self.rows_found(lines).is_empty()
    }
}

pub(crate) enum Verdict {
    Met,
    Pending(Option<Instant>),
    Never,
}

pub(crate) struct Seen<'a> {
    pub screen: &'a Screen,
    pub frame: u64,
    pub last_change: Instant,
    pub last_output: Instant,
    pub first_output: Option<Instant>,
    pub exited: bool,
    pub prompt: Option<PromptState>,
}

pub(crate) struct Moment<'a> {
    now: Instant,
    seen: Seen<'a>,
    view: OnceCell<ScreenView>,
}

impl<'a> Moment<'a> {
    pub(crate) fn new(seen: Seen<'a>) -> Self {
        Self { now: Instant::now(), seen, view: OnceCell::new() }
    }

    pub(crate) fn now(&self) -> Instant {
        self.now
    }

    fn view(&self) -> &ScreenView {
        self.view.get_or_init(|| self.seen.screen.view())
    }
}

impl Condition {
    pub(crate) fn new(wait: &Wait) -> Result<Self, String> {
        Ok(match wait {
            Wait::Text(text) => Condition::Text(Pattern::new(text)?),
            Wait::Gone(text) => Condition::Gone(Pattern::new(text)?),
            Wait::Stable { quiet_ms } => Condition::Stable(Duration::from_millis(*quiet_ms), None),
            Wait::Idle { quiet_ms } => Condition::Idle(Duration::from_millis(*quiet_ms), None),
            Wait::Exit { .. } => Condition::Exit,
            Wait::Cursor { row, col } => Condition::Cursor(*row, *col),
            Wait::Frame { frame } => Condition::Frame(*frame),
            Wait::Elapsed { ms } => Condition::Elapsed(Instant::now() + Duration::from_millis(*ms)),
            Wait::Command => Condition::Command,
        })
    }

    pub(crate) fn counted_from(self, action: Instant) -> Self {
        match self {
            Condition::Stable(quiet, _) => Condition::Stable(quiet, Some(action)),
            Condition::Idle(quiet, _) => Condition::Idle(quiet, Some(action)),
            other => other,
        }
    }

    pub(crate) fn after_launch(self, launched: Instant, timeout: Duration) -> Self {
        match self {
            Condition::Stable(quiet, _) => {
                Condition::FirstStable { quiet, silent_by: launched + quiet.max(FIRST_OUTPUT_GRACE.min(timeout)) }
            }
            other => other.counted_from(launched),
        }
    }

    pub(crate) fn judge(&self, moment: &Moment<'_>, deadline: Instant) -> Verdict {
        let now = moment.now().min(deadline);
        match self {
            Condition::Text(pattern) => while_running(pattern.found(&moment.view().lines), moment),
            Condition::Gone(pattern) => while_running(!pattern.found(&moment.view().lines), moment),
            Condition::Stable(quiet, since) => {
                quiet_for(latest(moment.seen.last_change, *since) + *quiet, now, moment.seen.exited)
            }
            Condition::FirstStable { quiet, silent_by } => {
                let until =
                    moment.seen.first_output.map_or(*silent_by, |first| first.max(moment.seen.last_change) + *quiet);
                quiet_for(until, now, moment.seen.exited)
            }
            Condition::Idle(quiet, since) => {
                quiet_for(latest(moment.seen.last_output, *since) + *quiet, now, moment.seen.exited)
            }
            Condition::Exit => while_running(moment.seen.exited, moment),
            Condition::Cursor(row, col) => {
                let (line, column) = moment.view().cursor;
                while_running((line + 1, column + 1) == (*row, *col), moment)
            }
            Condition::Frame(frame) => while_running(moment.seen.frame >= *frame, moment),
            Condition::Elapsed(until) if now >= *until => Verdict::Met,
            Condition::Elapsed(until) => Verdict::Pending(Some(*until)),
            Condition::Command => while_running(moment.seen.prompt.is_some_and(|prompt| prompt.ready), moment),
            Condition::PromptAfter(prompts) => while_running(
                moment.seen.prompt.is_some_and(|prompt| prompt.ready && prompt.prompts > *prompts),
                moment,
            ),
        }
    }
}

fn while_running(met: bool, moment: &Moment<'_>) -> Verdict {
    if met {
        Verdict::Met
    } else if moment.seen.exited {
        Verdict::Never
    } else {
        Verdict::Pending(None)
    }
}

fn latest(last: Instant, since: Option<Instant>) -> Instant {
    since.map_or(last, |since| since.max(last))
}

fn quiet_for(until: Instant, now: Instant, exited: bool) -> Verdict {
    if exited || now >= until { Verdict::Met } else { Verdict::Pending(Some(until)) }
}

#[cfg(test)]
mod tests;
