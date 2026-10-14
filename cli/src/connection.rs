use std::io::{BufRead, BufReader, ErrorKind};
use std::os::fd::RawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use nix::fcntl::{FcntlArg, FdFlag, fcntl};

use crate::protocol::{self, Call, Reply, Response};

pub const READY: &str = "ready";

pub enum IfNotRunning {
    Start(Launch),
    Answer(Reply),
}

pub struct Launch {
    pub idle_timeout: Duration,
}

pub fn home() -> Result<PathBuf> {
    if let Some(home) = std::env::var_os("AGENT_TERMINAL_HOME") {
        return Ok(PathBuf::from(home));
    }
    let user_home = std::env::var_os("HOME").ok_or_else(|| {
        anyhow!("HOME is not set; set AGENT_TERMINAL_HOME to choose where agent-terminal keeps its files")
    })?;
    Ok(PathBuf::from(user_home).join(".agent-terminal"))
}

pub fn socket_path() -> Result<PathBuf> {
    Ok(home()?.join("daemon.sock"))
}

pub fn send(call: &Call, if_not_running: IfNotRunning) -> Result<Response> {
    let socket = socket_path()?;
    let mut stream = match UnixStream::connect(&socket) {
        Ok(stream) => stream,
        Err(e) if is_not_running(&e) => match if_not_running {
            IfNotRunning::Answer(reply) => return Ok(Response::Ok(reply)),
            IfNotRunning::Start(launch) => connect_or_start(&launch)?,
        },
        Err(e) => return Err(e).with_context(|| format!("connect to the daemon at {}", socket.display())),
    };
    protocol::write(&mut stream, call)?;
    protocol::read(&mut stream)?.context("the daemon closed the connection without replying")
}

fn connect_or_start(launch: &Launch) -> Result<UnixStream> {
    let home = home()?;
    let _lock = start_lock(&home)?;
    match UnixStream::connect(socket_path()?) {
        Ok(stream) => Ok(stream),
        Err(e) if is_not_running(&e) => start_daemon(&home, launch),
        Err(e) => Err(e).context("connect to the daemon"),
    }
}

fn start_lock(home: &std::path::Path) -> Result<std::fs::File> {
    std::fs::create_dir_all(home).with_context(|| format!("create {}", home.display()))?;
    let lock_path = home.join("start.lock");
    let lock = std::fs::File::create(&lock_path).with_context(|| format!("create {}", lock_path.display()))?;
    lock.lock().with_context(|| format!("lock {}", lock_path.display()))?;
    Ok(lock)
}

pub fn remove_stale_socket() -> Result<bool> {
    let _lock = start_lock(&home()?)?;
    let socket = socket_path()?;
    match UnixStream::connect(&socket) {
        Err(e) if e.kind() == ErrorKind::ConnectionRefused => {
            std::fs::remove_file(&socket).with_context(|| format!("remove {}", socket.display()))?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn is_not_running(error: &std::io::Error) -> bool {
    matches!(error.kind(), ErrorKind::NotFound | ErrorKind::ConnectionRefused)
}

fn start_daemon(home: &std::path::Path, launch: &Launch) -> Result<UnixStream> {
    let log_path = home.join("daemon.log");
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("open {}", log_path.display()))?;
    keep_inherited_descriptors_from_the_daemon()?;
    let mut daemon = Command::new(std::env::current_exe()?)
        .args(["daemon", "run", "--idle-timeout", &launch.idle_timeout.as_millis().to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(log)
        .process_group(0)
        .spawn()
        .context("start the agent-terminal daemon")?;

    let mut announcement = String::new();
    BufReader::new(daemon.stdout.take().context("daemon stdout")?)
        .read_line(&mut announcement)
        .context("wait for the daemon to start")?;
    if announcement.trim() != READY {
        bail!("the daemon did not start; see {}", log_path.display());
    }
    UnixStream::connect(socket_path()?).context("connect to the daemon")
}

fn keep_inherited_descriptors_from_the_daemon() -> Result<()> {
    let listing = "list the open descriptors in /dev/fd";
    for entry in std::fs::read_dir("/dev/fd").context(listing)? {
        let name = entry.context(listing)?.file_name();
        let descriptor: RawFd = name
            .to_str()
            .and_then(|number| number.parse().ok())
            .with_context(|| format!("/dev/fd lists {name:?}, which is not a descriptor number"))?;
        if descriptor > 2 {
            fcntl(descriptor, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC))
                .with_context(|| format!("mark descriptor {descriptor} close-on-exec"))?;
        }
    }
    Ok(())
}
