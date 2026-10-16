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
    let new_session = opening.new_session;
    let (session, request) = match opening.request() {
        Ok(asked) => asked,
        Err(message) => return refused(message),
    };
    if new_session { open_in_new_session(backend, session, request).await } else { call(backend, request).await }
}

async fn open_in_new_session(backend: Arc<dyn Backend>, session: Name, request: Request) -> Response {
    let created =
        call(Arc::clone(&backend), Request::NewSession(request::NewSession { session: session.clone() })).await;
    if !created.status().is_success() {
        return created;
    }
    let opened = call(Arc::clone(&backend), request).await;
    if !opened.status().is_success() {
        call(backend, Request::Close(request::Close::Session { session })).await;
    }
    opened
}

impl Opening {
    fn request(self) -> Result<(Name, Request), String> {
        let mut words = shlex::split(&self.command).ok_or("a quote in the command is not closed")?.into_iter();
        let program = words.next().ok_or("enter a command")?;
        let session: Name = self.session.parse()?;
        let tab: Name = self.tab.parse()?;
        let size = Size::new(self.cols, self.rows)?;
        let cwd = working_directory(&self.cwd)?;
        let env = pty::inherited(std::env::vars());
        if !self.shell_integration {
            let open = Request::Open(request::Open {
                session: session.clone(),
                tab,
                program,
                args: words.collect(),
                size,
                cwd,
                env,
                wait: None,
                timeout_ms: 0,
            });
            return Ok((session, open));
        }
        let shell = Shell::of_program(&program)
            .ok_or_else(|| format!("{program} has no shell integration; it comes with zsh, bash and fish"))?;
        let shell = Request::Shell(request::OpenShell {
            session: session.clone(),
            tab,
            shell,
            program,
            size,
            cwd,
            env,
            wait: None,
            timeout_ms: 0,
        });
        Ok((session, shell))
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
