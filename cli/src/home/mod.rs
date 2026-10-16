use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use nix::unistd::{User, getuid};

pub(crate) fn home() -> Result<PathBuf> {
    #[cfg(any(debug_assertions, feature = "private-daemon"))]
    if let Some(home) = std::env::var_os("AGENT_TERMINAL_HOME") {
        return Ok(PathBuf::from(home));
    }
    user_home()
}

fn user_home() -> Result<PathBuf> {
    let uid = getuid();
    let user =
        User::from_uid(uid).with_context(|| format!("look up the user account of uid {uid}"))?.ok_or_else(|| {
            anyhow!("no user account has uid {uid}, so agent-terminal has no folder to keep its files in")
        })?;
    Ok(user.dir.join(".agent-terminal"))
}

pub(crate) const SOCKET: &str = "daemon.sock";

pub(crate) const DAEMON_VERSION: &str = "daemon.version";

pub(crate) const DAEMON_LOCK: &str = "daemon.lock";

pub(crate) fn socket_path() -> Result<PathBuf> {
    Ok(home()?.join(SOCKET))
}

pub(crate) fn running_build(home: &Path) -> Result<Option<String>> {
    let path = home.join(DAEMON_VERSION);
    match std::fs::read_to_string(&path) {
        Ok(build) => Ok(Some(build.trim().to_string())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("read the daemon's build from {}", path.display())),
    }
}

#[cfg(test)]
mod tests;
