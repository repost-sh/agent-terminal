use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::protocol::{self, Request, Response};

pub fn home() -> PathBuf {
    if let Some(home) = std::env::var_os("AGENT_TERMINAL_HOME") {
        return PathBuf::from(home);
    }
    let user_home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
    user_home.join(".agent-terminal")
}

pub fn socket_path() -> PathBuf {
    home().join("daemon.sock")
}

pub fn send(request: &Request, start: bool) -> Result<Option<Response>> {
    let mut stream = match UnixStream::connect(socket_path()) {
        Ok(stream) => stream,
        Err(_) if !start => return Ok(None),
        Err(_) => start_daemon()?,
    };
    protocol::write(&mut stream, request)?;
    Ok(Some(protocol::read(&mut stream).context("read the daemon's reply")?))
}

fn start_daemon() -> Result<UnixStream> {
    let home = home();
    std::fs::create_dir_all(&home).with_context(|| format!("create {}", home.display()))?;
    let log = std::fs::File::create(home.join("daemon.log"))?;
    Command::new(std::env::current_exe()?)
        .args(["daemon", "run"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log)
        .process_group(0)
        .spawn()
        .context("start the agent-terminal daemon")?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(stream) = UnixStream::connect(socket_path()) {
            return Ok(stream);
        }
        if Instant::now() >= deadline {
            bail!("the daemon did not start; see {}", home.join("daemon.log").display());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
