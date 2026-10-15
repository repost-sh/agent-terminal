use std::path::PathBuf;
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::response::Response;
use serde::Deserialize;

use super::Backend;
use super::api::{call, refused};
use crate::native::pty;
use crate::protocol::request;
use crate::protocol::{Request, Sessions, Shell, Size};

#[derive(Deserialize)]
pub(super) struct Opening {
    session: String,
    new_session: bool,
    command: String,
    cwd: String,
    cols: u16,
    rows: u16,
    shell_integration: bool,
}

pub(super) async fn open(State(backend): State<Arc<dyn Backend>>, Json(opening): Json<Opening>) -> Response {
    match opening.request(&backend.sessions()) {
        Ok(request) => call(backend, request).await,
        Err(message) => refused(message),
    }
}

impl Opening {
    fn request(self, open: &Sessions) -> Result<Request, String> {
        let mut words = shlex::split(&self.command).ok_or("a quote in the command is not closed")?.into_iter();
        let program = words.next().ok_or("enter a command")?;
        if self.new_session && open.sessions.iter().any(|entry| entry.session == self.session) {
            return Err(format!("A session named {} already exists.", self.session));
        }
        let size = Size::new(self.cols, self.rows)?;
        let cwd = working_directory(&self.cwd)?;
        let env = pty::inherited(std::env::vars());
        if !self.shell_integration {
            return Ok(Request::Open(request::Open {
                session: self.session,
                program,
                args: words.collect(),
                size,
                cwd,
                env,
                wait: None,
                timeout_ms: 0,
            }));
        }
        let shell = Shell::of_program(&program)
            .ok_or_else(|| format!("{program} has no shell integration; it comes with zsh, bash and fish"))?;
        Ok(Request::Shell(request::OpenShell {
            session: self.session,
            shell,
            program,
            size,
            cwd,
            env,
            wait: None,
            timeout_ms: 0,
        }))
    }
}

fn working_directory(typed: &str) -> Result<PathBuf, String> {
    let typed = typed.trim();
    if typed.starts_with('/') {
        return Ok(PathBuf::from(typed));
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| format!("HOME is not set, so {typed:?} has no home to start from; give an absolute path"))?;
    let relative = match typed.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => rest.trim_start_matches('/'),
        _ => typed,
    };
    Ok(if relative.is_empty() { home } else { home.join(relative) })
}
