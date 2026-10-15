use std::sync::Arc;

use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use tokio::sync::watch;
use tokio::time::Instant;

use super::Backend;
use super::stream::hung_up;
use crate::native::terminal::Terminal;
use crate::protocol::TabId;

type Lookup<T> = Box<dyn Fn() -> Option<Arc<T>> + Send + Sync>;

pub(super) enum Event {
    Woke,
    Received(Utf8Bytes),
    HungUp,
}

pub(super) enum Wake {
    OnChange,
    At(Instant),
    OnMessage,
}

pub(super) struct Follow<T> {
    lookup: Lookup<T>,
    signal: fn(&T) -> Option<watch::Receiver<u64>>,
    lifecycle: watch::Receiver<u64>,
    extra: Option<watch::Receiver<u64>>,
    watched: Option<(Arc<T>, Option<watch::Receiver<u64>>)>,
}

impl Follow<Terminal> {
    pub(super) fn tab(
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
    pub(super) fn new(
        backend: &Arc<dyn Backend>,
        lookup: Lookup<T>,
        signal: fn(&T) -> Option<watch::Receiver<u64>>,
    ) -> Self {
        Self { lookup, signal, lifecycle: backend.changes(), extra: None, watched: None }
    }

    pub(super) fn and(mut self, extra: Option<watch::Receiver<u64>>) -> Self {
        self.extra = extra;
        self
    }

    pub(super) fn next(&mut self) -> Option<(Arc<T>, bool)> {
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

    pub(super) async fn changed(&mut self, socket: &mut WebSocket) -> bool {
        !matches!(self.event(socket, Wake::OnChange).await, Event::HungUp)
    }

    pub(super) async fn event(&mut self, socket: &mut WebSocket, wake: Wake) -> Event {
        let Self { lifecycle, extra, watched, .. } = self;
        let listening = matches!(wake, Wake::OnChange);
        let signal = changed_or_never(watched.as_mut().and_then(|(_, signal)| signal.as_mut()).filter(|_| listening));
        let extra = changed_or_never(extra.as_mut().filter(|_| listening));
        let deadline = sleep_until_or_never(wake.deadline());
        let changed = first(first(lifecycle.changed(), signal), extra);
        let woken = first(async { woke(changed.await) }, async {
            deadline.await;
            Event::Woke
        });
        first(woken, async { from_socket(socket.recv().await) }).await
    }
}

pub(super) async fn first<T>(one: impl Future<Output = T>, other: impl Future<Output = T>) -> T {
    tokio::select! {
        done = one => done,
        done = other => done,
    }
}

impl Wake {
    fn deadline(&self) -> Option<Instant> {
        match self {
            Wake::At(deadline) => Some(*deadline),
            Wake::OnChange | Wake::OnMessage => None,
        }
    }
}

fn from_socket(received: Option<Result<Message, axum::Error>>) -> Event {
    match received {
        Some(Ok(Message::Text(text))) => Event::Received(text),
        received if hung_up(received.as_ref()) => Event::HungUp,
        _ => Event::Woke,
    }
}

fn woke(changed: Result<(), watch::error::RecvError>) -> Event {
    changed.map_or(Event::HungUp, |()| Event::Woke)
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
