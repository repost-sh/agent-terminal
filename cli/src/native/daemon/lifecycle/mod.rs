use std::sync::PoisonError;
use std::time::{Duration, Instant};

use super::Daemon;
use crate::home::{DAEMON_VERSION, SOCKET};
use crate::native::log;
use crate::protocol::{KeptRunning, Reply};

pub(super) const OTHER_CALLS_WITHIN: Duration = Duration::from_secs(5);

pub(super) struct Activity {
    in_flight: usize,
    last: Instant,
    stopping: bool,
}

impl Default for Activity {
    fn default() -> Self {
        Self { in_flight: 0, last: Instant::now(), stopping: false }
    }
}

pub(super) struct Busy<'a>(&'a Daemon);

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        let mut activity = self.0.activity.lock().unwrap_or_else(PoisonError::into_inner);
        activity.in_flight -= 1;
        activity.last = Instant::now();
        self.0.quiet.notify_all();
    }
}

impl Daemon {
    pub(super) fn busy(&self) -> Option<Busy<'_>> {
        let mut activity = self.activity.lock().unwrap_or_else(PoisonError::into_inner);
        if activity.stopping {
            return None;
        }
        activity.in_flight += 1;
        Some(Busy(self))
    }

    pub(super) fn stop_when_idle(&self, timeout: Duration) {
        self.wait_until_idle(timeout);
        log::note(format_args!("no call for {} ms, stopping", timeout.as_millis()));
        self.shut_down();
        self.end();
    }

    fn wait_until_idle(&self, timeout: Duration) {
        let mut activity = self.activity.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            let quiet_for = activity.last.elapsed();
            if activity.in_flight == 0 && quiet_for >= timeout {
                break;
            }
            activity = if activity.in_flight > 0 {
                self.quiet.wait(activity).unwrap_or_else(PoisonError::into_inner)
            } else {
                self.quiet
                    .wait_timeout(activity, timeout.saturating_sub(quiet_for))
                    .unwrap_or_else(PoisonError::into_inner)
                    .0
            };
        }
        activity.stopping = true;
    }

    pub(super) fn stop_if_no_terminals(&self, other_calls_within: Duration) -> Reply {
        let activity = self.activity.lock().unwrap_or_else(PoisonError::into_inner);
        let (mut activity, _) = self
            .quiet
            .wait_timeout_while(activity, other_calls_within, |activity| {
                activity.in_flight > 1 && self.terminals() == 0
            })
            .unwrap_or_else(PoisonError::into_inner);
        let terminals = self.terminals();
        if terminals > 0 {
            return Reply::KeptRunning(KeptRunning { terminals });
        }
        activity.stopping = true;
        drop(activity);
        log::note(format_args!("a command of another build stopped this daemon, which had no terminals"));
        self.stop()
    }

    pub(super) fn stop(&self) -> Reply {
        self.shut_down();
        Reply::Stopped
    }

    pub(super) fn shut_down(&self) {
        self.stop_dashboard();
        if let Err(e) = std::fs::remove_file(self.home.join(DAEMON_VERSION)) {
            log::note(format_args!("removing the version file failed: {e:#}"));
        }
        if let Err(e) = std::fs::remove_file(self.home.join(SOCKET)) {
            log::note(format_args!("removing the socket failed: {e:#}"));
        }
        self.close_all();
        if let Err(e) = self.claim.unlock() {
            log::note(format_args!("releasing the daemon's lock failed: {e:#}"));
        }
    }
}

#[cfg(test)]
mod tests;
