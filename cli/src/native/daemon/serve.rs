use std::os::unix::net::UnixStream;

use super::Daemon;
use super::refusals::stopping;
use crate::connection::daemon_stop_advice;
use crate::native::log;
use crate::output;
use crate::protocol::{self, Call, ErrorBody, OtherVersion, Reply, Request, Response};

impl Daemon {
    pub(super) fn serve(&self, mut stream: UnixStream) {
        let busy = self.busy();
        let body = match protocol::read_body(&mut stream) {
            Ok(Some(body)) => body,
            Ok(None) => return,
            Err(e) => {
                log::note(format_args!("reading a request failed: {e:#}"));
                return;
            }
        };
        let call: Call = match serde_json::from_slice(&body) {
            Ok(call) => call,
            Err(e) => return self.not_understood(stream, &e),
        };
        let request = match call.request {
            Request::Attach(attach) if busy.is_some() => return self.attach(stream, &attach.target, attach.read_only),
            request => request,
        };
        let response = self.respond(request, call.typed, busy.is_some());
        if let Err(e) = protocol::write(&mut stream, &response) {
            log::note(format_args!("sending a reply failed: {e:#}"));
        }
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

    fn not_understood(&self, mut stream: UnixStream, error: &serde_json::Error) {
        log::note(format_args!("a request this daemon does not understand: {error}"));
        let terminals = self.terminals();
        let response = Response::Err(ErrorBody::other_version(
            format!(
                "the daemon is agent-terminal {} and does not understand this request, so the command is another version; it has {}: {}",
                crate::VERSION,
                output::count(terminals, "open terminal"),
                daemon_stop_advice(None)
            ),
            OtherVersion {
                cli_version: None,
                daemon_version: Some(crate::VERSION.to_string()),
                terminals: Some(terminals),
            },
        ));
        if let Err(e) = protocol::write(&mut stream, &response) {
            log::note(format_args!("sending a reply failed: {e:#}"));
        }
    }
}
