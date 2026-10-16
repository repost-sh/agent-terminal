use std::os::unix::net::UnixStream;

use super::Daemon;
use super::refusals::stopping;
use crate::native::log;
use crate::output;
use crate::protocol::{self, Call, ErrorBody, OtherVersion, Reply, Request, Response, daemon_stop_advice};

impl Daemon {
    pub(super) fn serve(&self, mut stream: UnixStream) {
        let busy = self.busy();
        let Some(call) = self.read_call(&mut stream) else {
            return;
        };
        match call.request {
            Request::Attach(attach) if busy.is_some() => self.attach(stream, &attach),
            request => self.answer(&mut stream, request, call.typed, busy.is_some()),
        }
    }

    fn read_call(&self, stream: &mut UnixStream) -> Option<Call> {
        let body = match protocol::read_body(stream) {
            Ok(body) => body?,
            Err(e) => {
                log::note(format_args!("reading a request failed: {e:#}"));
                return None;
            }
        };
        match serde_json::from_slice(&body) {
            Ok(call) => Some(call),
            Err(e) => {
                self.not_understood(stream, &e);
                None
            }
        }
    }

    fn answer(&self, stream: &mut UnixStream, request: Request, typed: Option<Vec<String>>, busy: bool) {
        let response = self.respond(request, typed, busy);
        send(stream, &response);
        if matches!(response, Response::Ok(Reply::Stopped)) {
            self.end();
        }
    }

    fn respond(&self, request: Request, typed: Option<Vec<String>>, busy: bool) -> Response {
        let logged = typed.and_then(|typed| self.about_to_handle(typed, &request));
        let handled = if busy {
            self.handle(request, logged.as_ref().map(|logged| logged.typed.as_slice()))
        } else {
            Err(stopping())
        };
        if let Some(logged) = logged {
            self.log(logged, &handled);
        }
        match handled {
            Ok(reply) => Response::Ok(reply),
            Err(failure) => Response::Err(failure),
        }
    }

    fn not_understood(&self, stream: &mut UnixStream, error: &serde_json::Error) {
        log::note(format_args!("a request this daemon does not understand: {error}"));
        let terminals = self.terminals();
        let response = Response::Err(ErrorBody::other_version(
            format!(
                "the daemon is agent-terminal {} and does not understand this request, so the command is another version; it has {}: {}",
                crate::version::VERSION,
                output::count(terminals, "open terminal"),
                daemon_stop_advice(None)
            ),
            OtherVersion {
                cli_version: None,
                daemon_version: Some(crate::version::VERSION.to_string()),
                terminals: Some(terminals),
            },
        ));
        send(stream, &response);
    }
}

fn send(stream: &mut UnixStream, response: &Response) {
    if let Err(e) = protocol::write(stream, response) {
        log::note(format_args!("sending a reply failed: {e:#}"));
    }
}
