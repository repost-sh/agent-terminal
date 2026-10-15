use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use tokio::sync::watch;
use tokio::time::Instant;

use super::Backend;
use super::stream::hung_up;
use crate::native::terminal::Terminal;
use crate::protocol::TabId;

type Lookup<T> = Box<dyn Fn() -> Option<Arc<T>> + Send + Sync>;

pub enum Event {
    Changed,
    Received(String),
    Ignored,
    Deadline,
    HungUp,
}

pub struct Follow<T> {
    lookup: Lookup<T>,
    signal: fn(&T) -> Option<watch::Receiver<u64>>,
    lifecycle: watch::Receiver<u64>,
    extra: Option<watch::Receiver<u64>>,
    watched: Option<(Arc<T>, Option<watch::Receiver<u64>>)>,
}

impl Follow<Terminal> {
    pub fn tab(
        backend: &Arc<dyn Backend>,
        session: String,
        id: TabId,
        signal: fn(&Terminal) -> Option<watch::Receiver<u64>>,
    ) -> Self {
        let looking = Arc::clone(backend);
        Self::new(backend, Box::new(move || looking.terminal(&session, id)), signal)
    }
}

impl<T> Follow<T> {
    pub fn new(backend: &Arc<dyn Backend>, lookup: Lookup<T>, signal: fn(&T) -> Option<watch::Receiver<u64>>) -> Self {
        Self { lookup, signal, lifecycle: backend.changes(), extra: None, watched: None }
    }

    pub fn and(mut self, extra: Option<watch::Receiver<u64>>) -> Self {
        self.extra = extra;
        self
    }

    pub fn next(&mut self) -> Option<(Arc<T>, bool)> {
        self.lifecycle.borrow_and_update();
        if let Some(extra) = &mut self.extra {
            extra.borrow_and_update();
        }
        let current = (self.lookup)()?;
        let replaced = !self.watched.as_ref().is_some_and(|(watching, _)| Arc::ptr_eq(watching, &current));
        if replaced {
            self.watched = Some((Arc::clone(&current), (self.signal)(&current)));
        }
        if let Some((_, Some(signal))) = &mut self.watched {
            signal.borrow_and_update();
        }
        Some((current, replaced))
    }

    pub async fn changed(&mut self, socket: &mut WebSocket) -> bool {
        let Self { lifecycle, extra, watched, .. } = self;
        let signal = changed_or_never(watched.as_mut().and_then(|(_, signal)| signal.as_mut()));
        let extra = changed_or_never(extra.as_mut());
        tokio::select! {
            changed = lifecycle.changed() => changed.is_ok(),
            changed = signal => changed.is_ok(),
            changed = extra => changed.is_ok(),
            received = socket.recv() => !hung_up(received),
        }
    }

    pub async fn event(&mut self, socket: &mut WebSocket, deadline: Option<Instant>) -> Event {
        let Self { lifecycle, extra, watched, .. } = self;
        let signal = changed_or_never(watched.as_mut().and_then(|(_, signal)| signal.as_mut()));
        let extra = changed_or_never(extra.as_mut());
        tokio::select! {
            changed = lifecycle.changed() => change(changed),
            changed = signal => change(changed),
            changed = extra => change(changed),
            received = socket.recv() => match received {
                Some(Ok(Message::Text(text))) => Event::Received(text.to_string()),
                received => if hung_up(received) { Event::HungUp } else { Event::Ignored },
            },
            () = sleep_until_or_never(deadline) => Event::Deadline,
        }
    }
}

fn change(changed: Result<(), watch::error::RecvError>) -> Event {
    match changed {
        Ok(()) => Event::Changed,
        Err(_) => Event::HungUp,
    }
}

async fn sleep_until_or_never(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

async fn changed_or_never(receiver: Option<&mut watch::Receiver<u64>>) -> Result<(), watch::error::RecvError> {
    match receiver {
        Some(receiver) => receiver.changed().await,
        None => std::future::pending().await,
    }
}
