use std::path::PathBuf;

use anyhow::{Result, anyhow};

pub(crate) fn home() -> Result<PathBuf> {
    if let Some(home) = std::env::var_os("AGENT_TERMINAL_HOME") {
        return Ok(PathBuf::from(home));
    }
    let user_home = std::env::var_os("HOME").ok_or_else(|| {
        anyhow!("HOME is not set; set AGENT_TERMINAL_HOME to choose where agent-terminal keeps its files")
    })?;
    Ok(PathBuf::from(user_home).join(".agent-terminal"))
}

pub(crate) fn socket_path() -> Result<PathBuf> {
    Ok(home()?.join("daemon.sock"))
}

pub(crate) fn version_path() -> Result<PathBuf> {
    Ok(home()?.join("daemon.version"))
}
