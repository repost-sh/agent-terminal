use std::io::{ErrorKind, Read, Write};
use std::sync::{Arc, PoisonError};
use std::thread::JoinHandle;
use std::time::Instant;

use anyhow::Result;

use super::state::State;
use super::{Status, Terminal};
use crate::native::commands::{Change, Commands, Point};
use crate::native::log;
use crate::native::output_log::{Chunk, Offsets, OutputLog};
use crate::native::screen::{At, ShellReport};

type Wait = Box<dyn FnOnce() -> Result<i32> + Send>;

#[derive(Clone, Copy)]
struct Reported {
    start: Offsets,
    started: u64,
    shown: u64,
}

impl Terminal {
    pub(super) fn follow_output(self: &Arc<Self>, reader: Box<dyn Read + Send>, wait: Wait) {
        let reading = Arc::clone(self);
        let output_done = std::thread::spawn(move || reading.pump(reader));
        let waiting = Arc::clone(self);
        std::thread::spawn(move || waiting.wait_for_exit(wait, output_done));
    }

    fn pump(&self, mut reader: Box<dyn Read + Send>) {
        let mut buffer = vec![0u8; 64 * 1024];
        while let Some(read) = self.read_output(&mut reader, &mut buffer) {
            self.output(buffer.get(..read).unwrap_or_default());
        }
    }

    fn read_output(&self, reader: &mut Box<dyn Read + Send>, buffer: &mut [u8]) -> Option<usize> {
        loop {
            match reader.read(buffer) {
                Ok(0) => return None,
                Ok(read) => return Some(read),
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) if e.raw_os_error() == Some(libc::EIO) => return None,
                Err(e) => {
                    log::note(format_args!("{}: reading program output failed: {e}", self.program));
                    return None;
                }
            }
        }
    }

    fn wait_for_exit(&self, wait: Wait, output_done: JoinHandle<()>) {
        let code = wait()
            .inspect_err(|e| log::note(format_args!("{}: could not read the exit status: {e:#}", self.program)))
            .ok();
        if output_done.join().is_err() {
            log::note(format_args!("{}: the output reader panicked", self.program));
        }
        self.state.lock().unwrap_or_else(PoisonError::into_inner).status = Status::Exited(code);
        self.changed.notify_all();
        self.frames.notify();
        self.lifecycle.notify();
    }

    fn output(&self, bytes: &[u8]) {
        let (replies, history_changed) = self.take_in(bytes);
        self.changed.notify_all();
        self.frames.notify();
        if history_changed {
            self.history.notify();
        }
        if replies.is_empty() {
            return;
        }
        let mut writer = self.writer.lock().unwrap_or_else(PoisonError::into_inner);
        if let Err(e) = writer.write_all(&replies).and_then(|()| writer.flush()) {
            log::note(format_args!("{}: answering a terminal query failed: {e}", self.program));
        }
    }

    fn take_in(&self, bytes: &[u8]) -> (Vec<u8>, bool) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let replies = state.screen.feed(bytes);
        let printed = state.screen.take_printed();
        let reports = state.screen.take_shell_reports();
        let started = state.frame;
        let start = state.log.end();
        state
            .log
            .record(started, &Chunk { raw: bytes, text: printed.text.as_bytes(), styled: printed.styled.as_bytes() });
        let now = Instant::now();
        state.last_output = now;
        state.advance(now);
        let reported = Reported { start, started, shown: state.frame };
        let history_changed = self.report(&mut state, reports, reported);
        state.publish();
        (replies, history_changed)
    }

    fn report(&self, state: &mut State, reports: Vec<(At, ShellReport)>, reported: Reported) -> bool {
        let State { commands, log, .. } = state;
        let Some(commands) = commands else {
            return false;
        };
        let mut changed = false;
        for (at, report) in reports {
            let frame = if matches!(report, ShellReport::Started { .. }) { reported.started } else { reported.shown };
            let output = Offsets {
                text: reported.start.text + at.text as u64,
                styled: reported.start.styled + at.styled as u64,
            };
            let change = commands.report(report, Point { output, frame });
            changed |= self.note(commands, log, change);
        }
        changed
    }

    fn note(&self, commands: &Commands, log: &OutputLog, change: Option<Change>) -> bool {
        let Some(change) = change else {
            return false;
        };
        if let Change::Finished { id } = change
            && let Some((record, from, to)) = commands.record(id)
        {
            self.command_log.append(&record, &log.text_between(from, to));
        }
        true
    }
}
