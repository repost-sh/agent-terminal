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
use crate::protocol::{Name, Request, Shell, Size};

#[derive(Deserialize)]
pub(super) struct Opening {
    session: String,
    new_session: bool,
    tab: String,
    command: String,
    cwd: String,
    cols: u16,
    rows: u16,
    shell_integration: bool,
}

pub(super) async fn open(State(backend): State<Arc<dyn Backend>>, Json(opening): Json<Opening>) -> Response {
    match opening.request() {
        Ok(request) => call(backend, request).await,
        Err(message) => refused(message),
    }
}

impl Opening {
    fn request(self) -> Result<Request, String> {
        let mut words = shlex::split(&self.command).ok_or("a quote in the command is not closed")?.into_iter();
        let program = words.next().ok_or("enter a command")?;
        let session: Name = self.session.parse()?;
        let tab: Name = self.tab.parse()?;
        let size = Size::new(self.cols, self.rows)?;
        let cwd = working_directory(&self.cwd)?;
        let env = pty::inherited(std::env::vars());
        let new_session = self.new_session;
        if !self.shell_integration {
            return Ok(Request::Open(request::Open {
                session,
                new_session,
                tab,
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
            session,
            new_session,
            tab,
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
    let relative = under_home(typed);
    Ok(if relative.is_empty() { home } else { home.join(relative) })
}

fn under_home(typed: &str) -> &str {
    match typed.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => rest.trim_start_matches('/'),
        _ => typed,
    }
}

#[cfg(test)]
mod tests;
