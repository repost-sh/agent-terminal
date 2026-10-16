use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::watch;

use super::Daemon;
use crate::native::control::{Control, TabHold};
use crate::native::dashboard::Backend;
use crate::native::log;
use crate::native::terminal::{Picture, Status, Terminal};
use crate::protocol::{
    self, AttachEvent, AttachInput, Attached, HumanInput, Mirror, Reply, Response, Size, TabId, request,
};

struct Watched {
    terminal: Arc<Terminal>,
    frames: watch::Receiver<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Shown {
    frame: u64,
    status: Status,
    size: Size,
}

struct Mirroring<'a> {
    daemon: &'a Daemon,
    session: &'a str,
    id: TabId,
    control: &'a Arc<Control>,
    hold: Option<&'a TabHold>,
}

impl Daemon {
    pub(super) fn attach(&self, mut stream: UnixStream, attach: &request::Attach) {
        let request::Attach { target, read_only } = attach;
        let read_only = *read_only;
        let session = target.session.as_str();
        match self.tab(target) {
            Ok((id, attached)) => {
                let hold = (!read_only).then(|| TabHold {
                    session: session.to_string(),
                    tab: id,
                    hold: attached.control.take(),
                });
                let reply = Reply::Attached(Attached {
                    session: session.to_string(),
                    tab: id.to_string(),
                    program: attached.terminal.program().to_string(),
                    read_only,
                });
                let mirroring =
                    Mirroring { daemon: self, session, id, control: &attached.control, hold: hold.as_ref() };
                mirroring.serve(stream, reply);
            }
            Err(error) => {
                if let Err(e) = protocol::write(&mut stream, &Response::Err(error)) {
                    log::note(format_args!("attach: refusing the attach to {session} failed: {e:#}"));
                }
            }
        }
    }
}

impl Mirroring<'_> {
    fn serve(&self, mut stream: UnixStream, reply: Reply) {
        let (session, id) = (self.session, self.id);
        if let Err(e) = protocol::write(&mut stream, &Response::Ok(reply)) {
            log::note(format_args!("attach: answering the attach to {session} {id} failed: {e:#}"));
            return;
        }
        if let Err(e) = self.run(&stream) {
            log::note(format_args!("attach: mirroring {session} {id} ended: {e:#}"));
        }
        if let Err(e) = stream.shutdown(Shutdown::Both) {
            log::note(format_args!("attach: closing the connection to {session} {id} failed: {e}"));
        }
    }

    fn run(&self, stream: &UnixStream) -> Result<()> {
        let mut writer = stream.try_clone().context("share the attach connection")?;
        let reader = stream.try_clone().context("share the attach connection")?;
        let (inputs, mut received) = mpsc::unbounded_channel();
        std::thread::spawn(move || read_attach_inputs(reader, &inputs));
        let runtime = tokio::runtime::Builder::new_current_thread().build().context("start the attach runtime")?;
        runtime.block_on(self.mirror(&mut writer, &mut received))
    }

    async fn mirror(&self, writer: &mut UnixStream, received: &mut UnboundedReceiver<AttachInput>) -> Result<()> {
        let mut lifecycle = self.daemon.changes.subscribe();
        let mut watched: Option<Watched> = None;
        let mut sent = None;
        loop {
            lifecycle.borrow_and_update();
            let Some(terminal) = self.daemon.terminal_under(self.session, self.id, self.control) else {
                return protocol::write(writer, &AttachEvent::Closed);
            };
            let watching = watching(&mut watched, &terminal);
            watching.frames.borrow_and_update();
            sent = Some(show(writer, &terminal, sent)?);
            tokio::select! {
                changed = lifecycle.changed() => changed?,
                changed = watching.frames.changed() => changed?,
                input = received.recv() => match input {
                    Some(AttachInput::Human(input)) => self.typed(&input),
                    Some(AttachInput::Detach) => return protocol::write(writer, &AttachEvent::Detached),
                    None => return Ok(()),
                },
            }
        }
    }

    fn typed(&self, input: &HumanInput) {
        let Some(hold) = self.hold else {
            return;
        };
        if let Err(refused) = self.daemon.type_as_human(hold, input) {
            log::note(format_args!("attach: {}", refused.message));
        }
    }
}

fn watching<'a>(watched: &'a mut Option<Watched>, terminal: &Arc<Terminal>) -> &'a mut Watched {
    if !watched.as_ref().is_some_and(|watching| Arc::ptr_eq(&watching.terminal, terminal)) {
        *watched = None;
    }
    watched.get_or_insert_with(|| Watched { terminal: Arc::clone(terminal), frames: terminal.frames() })
}

fn show(writer: &mut UnixStream, terminal: &Terminal, sent: Option<Shown>) -> Result<Shown> {
    let picture = terminal.picture();
    let shown = Shown { frame: picture.glance.frame, status: picture.glance.status, size: picture.glance.size };
    if sent != Some(shown) {
        protocol::write(writer, &AttachEvent::Screen(mirrored(terminal.program(), picture)))?;
    }
    Ok(shown)
}

fn mirrored(program: &str, picture: Picture) -> Mirror {
    Mirror {
        program: program.to_string(),
        frame: picture.glance.frame,
        cols: picture.glance.size.cols,
        rows: picture.glance.size.rows,
        lines: picture.rows,
        cursor: [picture.cursor.0, picture.cursor.1],
        cursor_visible: picture.cursor_visible,
        exited: picture.glance.status.exited(),
        exit_code: picture.glance.status.code(),
    }
}

fn read_attach_inputs(mut reader: UnixStream, inputs: &UnboundedSender<AttachInput>) {
    loop {
        match protocol::read::<AttachInput>(&mut reader) {
            Ok(Some(input)) => {
                if inputs.send(input).is_err() {
                    return;
                }
            }
            Ok(None) => return,
            Err(e) => {
                log::note(format_args!("attach: reading the keys of an attached terminal failed: {e:#}"));
                return;
            }
        }
    }
}
