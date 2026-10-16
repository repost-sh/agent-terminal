use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::native::output_log::Offsets;
use crate::native::screen::ShellReport;
use crate::protocol::CommandRecord;

pub(crate) struct Commands {
    records: Vec<Record>,
    phase: Phase,
    entered: bool,
    prompts: u64,
    start_cwd: String,
    first_id: u32,
    next_id: u32,
    clock: Clock,
}

struct Clock {
    wall: SystemTime,
    origin: Instant,
}

impl Clock {
    fn start() -> Self {
        Self { wall: SystemTime::now(), origin: Instant::now() }
    }

    fn now(&self) -> (SystemTime, Instant) {
        let now = Instant::now();
        (self.wall + (now - self.origin), now)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Starting,
    Prompting,
    Ready,
    Running,
    Finished,
}

struct Record {
    command: String,
    cwd: String,
    started_at: SystemTime,
    started: Instant,
    ended: Option<(SystemTime, Instant)>,
    exit_code: Option<i32>,
    output_from: Offsets,
    output_to: Option<Offsets>,
    first_frame: u64,
    last_frame: Option<u64>,
}

#[derive(Clone, Copy)]
pub(crate) struct PromptState {
    pub ready: bool,
    pub prompts: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct Point {
    pub output: Offsets,
    pub frame: u64,
}

pub(crate) enum Change {
    Started,
    Finished { id: u32 },
}

pub(crate) struct Submitted {
    pub prompts: u64,
    pub first_id: u32,
}

impl Commands {
    pub(crate) fn new(start_cwd: String, first_id: u32) -> Self {
        Self {
            records: Vec::new(),
            phase: Phase::Starting,
            entered: false,
            prompts: 0,
            start_cwd,
            first_id,
            next_id: first_id,
            clock: Clock::start(),
        }
    }

    pub(crate) fn next_id(&self) -> u32 {
        self.next_id
    }

    pub(crate) fn report(&mut self, report: ShellReport, at: Point) -> Option<Change> {
        match report {
            ShellReport::Prompt => {
                let finished = (self.phase == Phase::Running).then(|| self.finish(None, &at));
                self.prompts += 1;
                self.entered = false;
                self.phase = Phase::Prompting;
                finished
            }
            ShellReport::Ready => {
                if self.phase != Phase::Running {
                    self.phase = Phase::Ready;
                }
                None
            }
            ShellReport::Started { .. } if self.phase == Phase::Running => None,
            ShellReport::Started { command, cwd } => {
                let (started_at, started) = self.clock.now();
                self.records.push(Record {
                    command,
                    cwd: cwd.unwrap_or_else(|| self.start_cwd.clone()),
                    started_at,
                    started,
                    ended: None,
                    exit_code: None,
                    output_from: at.output,
                    output_to: None,
                    first_frame: at.frame,
                    last_frame: None,
                });
                self.next_id += 1;
                self.entered = false;
                self.phase = Phase::Running;
                Some(Change::Started)
            }
            ShellReport::Finished { exit_code } if self.phase == Phase::Running => Some(self.finish(exit_code, &at)),
            ShellReport::Finished { .. } => None,
        }
    }

    fn finish(&mut self, exit_code: Option<i32>, at: &Point) -> Change {
        let ended = self.clock.now();
        if let Some(record) = self.records.last_mut() {
            record.ended = Some(ended);
            record.exit_code = exit_code;
            record.output_to = Some(at.output);
            record.last_frame = Some(at.frame);
        }
        self.phase = Phase::Finished;
        Change::Finished { id: self.next_id() - 1 }
    }

    pub(crate) fn entered(&mut self) {
        self.entered = true;
    }

    pub(crate) fn prompt(&self) -> PromptState {
        PromptState { ready: self.phase == Phase::Ready && !self.entered, prompts: self.prompts }
    }

    pub(crate) fn submitted(&self) -> Submitted {
        Submitted { prompts: self.prompts, first_id: self.next_id() }
    }

    pub(crate) fn running(&self) -> Option<&str> {
        match self.phase {
            Phase::Running => self.records.last().map(|record| record.command.as_str()),
            _ => None,
        }
    }

    pub(crate) fn last_exit_code(&self) -> Option<i32> {
        self.records.last().and_then(|record| record.exit_code)
    }

    pub(crate) fn records(&self) -> Vec<CommandRecord> {
        (self.first_id..).zip(&self.records).map(|(id, record)| record.public(id)).collect()
    }

    pub(crate) fn record(&self, id: u32) -> Option<(CommandRecord, Offsets, Option<Offsets>)> {
        let record = self.records.get(id.checked_sub(self.first_id)? as usize)?;
        Some((record.public(id), record.output_from, record.output_to))
    }
}

impl Record {
    fn public(&self, id: u32) -> CommandRecord {
        let ended_at = self.ended.map(|(at, _)| at);
        let duration = self.ended.map_or_else(|| self.started.elapsed(), |(_, ended)| ended - self.started);
        CommandRecord {
            id,
            command: self.command.clone(),
            cwd: self.cwd.clone(),
            started_at_ms: epoch_ms(self.started_at),
            ended_at_ms: ended_at.map(epoch_ms),
            duration_ms: u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
            exit_code: self.exit_code,
            first_frame: self.first_frame,
            last_frame: self.last_frame,
        }
    }
}

pub(crate) fn epoch_ms(at: SystemTime) -> u64 {
    at.duration_since(UNIX_EPOCH).map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests;
