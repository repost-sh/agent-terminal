use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant};

use super::Terminal;
use super::state::Watch;
use crate::native::wait::{Condition, Verdict};

pub(crate) enum Settled {
    Met { frame: u64 },
    Exited,
    TimedOut,
}

impl Terminal {
    pub(crate) fn wait(&self, condition: Condition, timeout: Duration) -> Settled {
        let deadline = Instant::now() + timeout;
        let condition = Arc::new(condition);
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let id = state.next_watch;
        state.next_watch += 1;
        state.watches.push(Watch { id, condition: Arc::clone(&condition), deadline, met: None });
        let settled = loop {
            if let Some(frame) = state.watches.iter().find(|watch| watch.id == id).and_then(|watch| watch.met) {
                break Settled::Met { frame };
            }
            let moment = state.moment();
            let wake = match condition.judge(&moment, deadline) {
                Verdict::Met => break Settled::Met { frame: state.frame },
                Verdict::Never => break Settled::Exited,
                Verdict::Pending(wake) => wake,
            };
            let now = moment.now();
            if now >= deadline {
                break Settled::TimedOut;
            }
            let until = wake.map_or(deadline, |wake| wake.min(deadline));
            state = self
                .changed
                .wait_timeout(state, until.saturating_duration_since(now))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        };
        state.watches.retain(|watch| watch.id != id);
        settled
    }
}
