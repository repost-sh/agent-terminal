use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use crate::native::changes::Changes;
use crate::protocol::Owner;

pub(crate) struct Control {
    holders: Mutex<Holders>,
    handed_back: Condvar,
    turn: Mutex<()>,
    lifecycle: Changes,
}

#[derive(Default)]
struct Holders {
    humans: usize,
    closed: bool,
}

pub(crate) struct Hold(Arc<Control>);

pub(crate) enum Handed {
    Back,
    TimedOut,
    Closed,
}

impl Control {
    pub(crate) fn new(lifecycle: Changes) -> Arc<Self> {
        Arc::new(Self {
            holders: Mutex::new(Holders::default()),
            handed_back: Condvar::new(),
            turn: Mutex::new(()),
            lifecycle,
        })
    }

    pub(crate) fn owner(&self) -> Owner {
        if self.holders.lock().unwrap_or_else(PoisonError::into_inner).humans > 0 { Owner::Human } else { Owner::Agent }
    }

    pub(crate) fn agent(&self) -> Option<MutexGuard<'_, ()>> {
        let turn = self.turn.lock().unwrap_or_else(PoisonError::into_inner);
        (self.owner() == Owner::Agent).then_some(turn)
    }

    pub(crate) fn take(self: &Arc<Self>) -> Hold {
        let turn = self.turn.lock().unwrap_or_else(PoisonError::into_inner);
        self.holders.lock().unwrap_or_else(PoisonError::into_inner).humans += 1;
        drop(turn);
        self.lifecycle.notify();
        Hold(Arc::clone(self))
    }

    pub(crate) fn wait_for_agent(&self, timeout: Duration) -> Handed {
        let holders = self.holders.lock().unwrap_or_else(PoisonError::into_inner);
        let (holders, _) = self
            .handed_back
            .wait_timeout_while(holders, timeout, |holders| holders.humans > 0 && !holders.closed)
            .unwrap_or_else(PoisonError::into_inner);
        if holders.closed {
            Handed::Closed
        } else if holders.humans > 0 {
            Handed::TimedOut
        } else {
            Handed::Back
        }
    }

    pub(crate) fn close(&self) {
        self.holders.lock().unwrap_or_else(PoisonError::into_inner).closed = true;
        self.handed_back.notify_all();
    }
}

impl Hold {
    pub(crate) fn holds(&self, control: &Arc<Control>) -> bool {
        Arc::ptr_eq(&self.0, control)
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        self.0.holders.lock().unwrap_or_else(PoisonError::into_inner).humans -= 1;
        self.0.handed_back.notify_all();
        self.0.lifecycle.notify();
    }
}
