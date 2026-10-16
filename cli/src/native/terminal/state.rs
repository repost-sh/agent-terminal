use std::sync::Arc;
use std::time::Instant;

use super::Status;
use crate::native::commands::Commands;
use crate::native::output_log::OutputLog;
use crate::native::refs::Found;
use crate::native::screen::Screen;
use crate::native::wait::{Condition, Moment, Seen, Verdict};
use crate::protocol::MouseButton;

pub(super) struct State {
    pub(super) screen: Screen,
    pub(super) frame: u64,
    pub(super) look: u64,
    pub(super) last_change: Instant,
    pub(super) last_output: Instant,
    pub(super) status: Status,
    pub(super) log: OutputLog,
    pub(super) last_shown: Option<Shown>,
    pub(super) listed: Option<Listed>,
    pub(super) mouse_held: Option<MouseButton>,
    pub(super) watches: Vec<Watch>,
    pub(super) next_watch: u64,
    pub(super) commands: Option<Commands>,
}

pub(super) struct Listed {
    pub(super) frame: u64,
    pub(super) refs: Vec<Found>,
}

pub(super) struct Shown {
    pub(super) frame: u64,
    pub(super) lines: Vec<String>,
}

pub(super) struct Watch {
    pub(super) id: u64,
    pub(super) condition: Arc<Condition>,
    pub(super) deadline: Instant,
    pub(super) met: Option<u64>,
}

impl State {
    pub(super) fn new(screen: Screen) -> Self {
        let started = Instant::now();
        Self {
            look: screen.fingerprint(),
            screen,
            frame: 0,
            last_change: started,
            last_output: started,
            status: Status::Running,
            log: OutputLog::default(),
            last_shown: None,
            listed: None,
            mouse_held: None,
            watches: Vec::new(),
            next_watch: 0,
            commands: None,
        }
    }

    pub(super) fn moment(&self) -> Moment<'_> {
        let prompt = self.commands.as_ref().map(Commands::prompt);
        Moment::new(Seen {
            screen: &self.screen,
            frame: self.frame,
            last_change: self.last_change,
            last_output: self.last_output,
            exited: self.status.exited(),
            prompt,
        })
    }

    pub(super) fn refresh(&mut self, now: Instant) {
        self.advance(now);
        self.publish();
    }

    pub(super) fn advance(&mut self, now: Instant) {
        let look = self.screen.fingerprint();
        if look != self.look {
            self.look = look;
            self.frame += 1;
            self.last_change = now;
        }
    }

    pub(super) fn publish(&mut self) {
        let State { screen, frame, last_change, last_output, status, watches, commands, .. } = self;
        if watches.iter().all(|watch| watch.met.is_some()) {
            return;
        }
        let prompt = commands.as_ref().map(Commands::prompt);
        let moment = Moment::new(Seen {
            screen,
            frame: *frame,
            last_change: *last_change,
            last_output: *last_output,
            exited: status.exited(),
            prompt,
        });
        for watch in watches.iter_mut().filter(|watch| watch.met.is_none()) {
            if let Verdict::Met = watch.condition.judge(&moment, watch.deadline) {
                watch.met = Some(*frame);
            }
        }
    }
}
