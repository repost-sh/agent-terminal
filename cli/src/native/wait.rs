use std::cell::OnceCell;
use std::time::{Duration, Instant};

use regex::Regex;

use crate::native::commands::PromptState;
use crate::native::screen::{Screen, ScreenView};
use crate::protocol::{RowRange, TextMatch, Wait};

pub enum Condition {
    Text(Pattern),
    Gone(Pattern),
    Stable(Duration, Option<Instant>),
    Idle(Duration, Option<Instant>),
    Exit,
    Cursor(u16, u16),
    Frame(u64),
    Elapsed(Instant),
    Command,
    PromptAfter(u64),
}

pub struct Pattern {
    regex: Regex,
    rows: Option<RowRange>,
}

pub fn compile(text: &str, regex: bool) -> Result<Regex, String> {
    let source = if regex { text.to_string() } else { regex::escape(text) };
    Regex::new(&source).map_err(|e| format!("invalid regex {text:?}: {e}"))
}

impl Pattern {
    pub fn new(text: &TextMatch) -> Result<Self, String> {
        Ok(Self { regex: compile(&text.text, text.regex)?, rows: text.rows })
    }

    pub fn rows_found(&self, lines: &[String]) -> Vec<u16> {
        let (first, last) = self.rows.map_or((1, lines.len()), |rows| (rows.first as usize, rows.last as usize));
        (first..=last.min(lines.len()))
            .filter(|&row| self.regex.is_match(&lines[row - 1]))
            .map(|row| row as u16)
            .collect()
    }

    pub fn found(&self, lines: &[String]) -> bool {
        !self.rows_found(lines).is_empty()
    }
}

pub enum Verdict {
    Met,
    Pending(Option<Instant>),
    Never,
}

pub struct Moment<'a> {
    pub now: Instant,
    screen: &'a Screen,
    frame: u64,
    last_change: Instant,
    last_output: Instant,
    exited: bool,
    prompt: Option<PromptState>,
    view: OnceCell<ScreenView>,
}

impl<'a> Moment<'a> {
    pub fn new(
        screen: &'a Screen,
        frame: u64,
        last_change: Instant,
        last_output: Instant,
        exited: bool,
        prompt: Option<PromptState>,
    ) -> Self {
        Self { now: Instant::now(), screen, frame, last_change, last_output, exited, prompt, view: OnceCell::new() }
    }

    pub fn by(mut self, deadline: Instant) -> Self {
        self.now = self.now.min(deadline);
        self
    }

    fn view(&self) -> &ScreenView {
        self.view.get_or_init(|| self.screen.view())
    }
}

impl Condition {
    pub fn new(wait: &Wait) -> Result<Self, String> {
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

    pub fn counted_from(self, action: Instant) -> Self {
        match self {
            Condition::Stable(quiet, _) => Condition::Stable(quiet, Some(action)),
            Condition::Idle(quiet, _) => Condition::Idle(quiet, Some(action)),
            other => other,
        }
    }

    pub fn judge(&self, moment: &Moment) -> Verdict {
        match self {
            Condition::Text(pattern) => while_running(pattern.found(&moment.view().lines), moment),
            Condition::Gone(pattern) => while_running(!pattern.found(&moment.view().lines), moment),
            Condition::Stable(quiet, since) => quiet_for(latest(moment.last_change, *since) + *quiet, moment),
            Condition::Idle(quiet, since) => quiet_for(latest(moment.last_output, *since) + *quiet, moment),
            Condition::Exit => while_running(moment.exited, moment),
            Condition::Cursor(row, col) => {
                let (line, column) = moment.view().cursor;
                while_running((line + 1, column + 1) == (*row, *col), moment)
            }
            Condition::Frame(frame) => while_running(moment.frame >= *frame, moment),
            Condition::Elapsed(until) if moment.now >= *until => Verdict::Met,
            Condition::Elapsed(until) => Verdict::Pending(Some(*until)),
            Condition::Command => while_running(moment.prompt.is_some_and(|prompt| prompt.ready), moment),
            Condition::PromptAfter(prompts) => {
                while_running(moment.prompt.is_some_and(|prompt| prompt.ready && prompt.prompts > *prompts), moment)
            }
        }
    }
}

fn while_running(met: bool, moment: &Moment) -> Verdict {
    if met {
        Verdict::Met
    } else if moment.exited {
        Verdict::Never
    } else {
        Verdict::Pending(None)
    }
}

fn latest(last: Instant, since: Option<Instant>) -> Instant {
    since.map_or(last, |since| since.max(last))
}

fn quiet_for(until: Instant, moment: &Moment) -> Verdict {
    if moment.exited || moment.now >= until { Verdict::Met } else { Verdict::Pending(Some(until)) }
}
