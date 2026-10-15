use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::os::fd::RawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use nix::fcntl::{FcntlArg, FdFlag, fcntl};

use crate::VERSION;
use crate::output::count;
use crate::protocol::{self, Call, ErrorBody, OtherVersion, Reply, Request, Response};

pub const READY: &str = "ready";
const STOPS_WITHIN: Duration = Duration::from_secs(10);

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

pub fn version_path() -> Result<PathBuf> {
    Ok(home()?.join("daemon.version"))
}

pub fn running_version() -> Result<Option<String>> {
    let path = version_path()?;
    match std::fs::read_to_string(&path) {
        Ok(version) => Ok(Some(version.trim().to_string())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("read the daemon's version from {}", path.display())),
    }
}

pub fn started_version() -> Result<Option<String>> {
    let running = running_version()?;
    if running.as_deref() == Some(VERSION) {
        return Ok(running);
    }
    let _lock = start_lock(&home()?)?;
    running_version()
}

pub fn daemon_stop_advice(cli_version: Option<&str>) -> String {
    let next = match cli_version {
        Some(version) => format!("a {version} daemon"),
        None => "a daemon of the command's version".to_string(),
    };
    format!("`agent-terminal daemon stop` ends it and its terminals, and the next command starts {next}")
}

pub fn daemon_of(version: Option<&str>) -> String {
    match version {
        Some(version) => format!("agent-terminal {version}"),
        None => "an agent-terminal that does not record its version".to_string(),
    }
}

enum Reached {
    Daemon(UnixStream),
    NotRunning,
    Refused(ErrorBody),
}

pub fn send(call: &Call, if_not_running: IfNotRunning) -> Result<Response> {
    let mut stream = match reach(&call.request)? {
        Reached::Daemon(stream) => stream,
        Reached::Refused(refusal) => return Ok(Response::Err(refusal)),
        Reached::NotRunning => match if_not_running {
            IfNotRunning::Answer(reply) => return Ok(Response::Ok(reply)),
            IfNotRunning::Start(launch) => connect_or_start(&launch)?,
        },
    };
    protocol::write(&mut stream, call)?;
    protocol::read(&mut stream)?.context("the daemon closed the connection without replying")
}

pub fn keep_open(call: &Call, launch: &Launch) -> Result<Result<(Response, UnixStream), ErrorBody>> {
    let mut stream = match reach(&call.request)? {
        Reached::Daemon(stream) => stream,
        Reached::Refused(refusal) => return Ok(Err(refusal)),
        Reached::NotRunning => connect_or_start(launch)?,
    };
    protocol::write(&mut stream, call)?;
    let response = protocol::read(&mut stream)?.context("the daemon closed the connection without replying")?;
    Ok(Ok((response, stream)))
}

fn reach(request: &Request) -> Result<Reached> {
    let Some(stream) = connect()? else {
        return Ok(Reached::NotRunning);
    };
    if matches!(request, Request::Stop) || running_version()?.as_deref() == Some(VERSION) {
        return Ok(Reached::Daemon(stream));
    }
    drop(stream);
    replace_other_version()
}

fn replace_other_version() -> Result<Reached> {
    let _lock = start_lock(&home()?)?;
    let Some(mut stream) = connect()? else {
        return Ok(Reached::NotRunning);
    };
    let running = running_version()?;
    if running.as_deref() == Some(VERSION) {
        return Ok(Reached::Daemon(stream));
    }
    let asking = || format!("ask the daemon ({}) to stop", daemon_of(running.as_deref()));
    stream.set_read_timeout(Some(STOPS_WITHIN)).with_context(asking)?;
    protocol::write(&mut stream, &Call { typed: None, request: Request::StopIfNoTerminals }).with_context(asking)?;
    match protocol::read(&mut stream).with_context(asking)? {
        Some(Response::Ok(Reply::Stopped)) => {
            let exit = || format!("wait for the daemon ({}) to exit", daemon_of(running.as_deref()));
            stream.read_to_end(&mut Vec::new()).with_context(exit)?;
            Ok(Reached::NotRunning)
        }
        Some(Response::Ok(Reply::KeptRunning(kept))) => {
            Ok(Reached::Refused(other_version(running.as_deref(), Some(kept.terminals))))
        }
        Some(Response::Ok(other)) => {
            bail!("{} answered the request to stop with {other:?}", daemon_of(running.as_deref()))
        }
        Some(Response::Err(error)) => Ok(Reached::Refused(error)),
        None => Ok(Reached::Refused(other_version(running.as_deref(), None))),
    }
}

fn other_version(running: Option<&str>, terminals: Option<usize>) -> ErrorBody {
    let open = match terminals {
        Some(terminals) => format!("it has {}", count(terminals, "open terminal")),
        None => "it did not say how many terminals it has open".to_string(),
    };
    ErrorBody {
        code: crate::exit::OTHER_VERSION,
        message: format!(
            "the daemon is {}, not {VERSION} like this command, and {open}, so it was left running; {}",
            daemon_of(running),
            daemon_stop_advice(Some(VERSION))
        ),
        snapshot: None,
        other_version: Some(OtherVersion {
            cli_version: Some(VERSION.to_string()),
            daemon_version: running.map(str::to_string),
            terminals,
        }),
    }
}

fn connect() -> Result<Option<UnixStream>> {
    let socket = socket_path()?;
    match UnixStream::connect(&socket) {
        Ok(stream) => Ok(Some(stream)),
        Err(e) if is_not_running(&e) => Ok(None),
        Err(e) => Err(e).with_context(|| format!("connect to the daemon at {}", socket.display())),
    }
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
