use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::os::fd::RawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use nix::fcntl::{FcntlArg, FdFlag, fcntl};

use crate::home::{home, running_build, socket_path};
use crate::native::daemon::READY;
use crate::output::count;
use crate::protocol::{self, Call, ErrorBody, OtherVersion, RESTART_ADVICE, Reply, Request, Response};
use crate::version::{BUILD, daemon_of};

const STOPS_WITHIN: Duration = Duration::from_secs(10);

pub(crate) enum IfNotRunning {
    Start,
    Answer(Box<Reply>),
}

enum Reached {
    Daemon(UnixStream),
    NotRunning,
    Refused(ErrorBody),
}

pub(crate) fn send(call: &Call, if_not_running: IfNotRunning) -> Result<Response> {
    let mut stream = match reach(&call.request)? {
        Reached::Daemon(stream) => stream,
        Reached::Refused(refusal) => return Ok(Response::Err(refusal)),
        Reached::NotRunning => match if_not_running {
            IfNotRunning::Answer(reply) => return Ok(Response::Ok(*reply)),
            IfNotRunning::Start => connect_or_start()?,
        },
    };
    protocol::write(&mut stream, call)?;
    protocol::read(&mut stream)?.context("the daemon closed the connection without replying")
}

pub(crate) fn keep_open(call: &Call) -> Result<Result<(Response, UnixStream), ErrorBody>> {
    let mut stream = match reach(&call.request)? {
        Reached::Daemon(stream) => stream,
        Reached::Refused(refusal) => return Ok(Err(refusal)),
        Reached::NotRunning => connect_or_start()?,
    };
    protocol::write(&mut stream, call)?;
    let response = protocol::read(&mut stream)?.context("the daemon closed the connection without replying")?;
    Ok(Ok((response, stream)))
}

fn reach(request: &Request) -> Result<Reached> {
    let Some(stream) = connect()? else {
        return Ok(Reached::NotRunning);
    };
    if matches!(request, Request::Stop) || running_build(&home()?)?.as_deref() == Some(BUILD) {
        return Ok(Reached::Daemon(stream));
    }
    drop(stream);
    replace_other_build()
}

fn replace_other_build() -> Result<Reached> {
    let _lock = start_lock(&home()?)?;
    let Some(mut stream) = connect()? else {
        return Ok(Reached::NotRunning);
    };
    let running = running_build(&home()?)?;
    if running.as_deref() == Some(BUILD) {
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
            Ok(Reached::Refused(other_build(running.as_deref(), Some(kept.terminals))))
        }
        Some(Response::Ok(other)) => {
            bail!("{} answered the request to stop with {other:?}", daemon_of(running.as_deref()))
        }
        Some(Response::Err(error)) => Ok(Reached::Refused(error)),
        None => Ok(Reached::Refused(other_build(running.as_deref(), None))),
    }
}

fn other_build(running: Option<&str>, terminals: Option<usize>) -> ErrorBody {
    let open = match terminals {
        Some(terminals) => format!("it has {}", count(terminals, "open terminal")),
        None => "it did not say how many terminals it has open".to_string(),
    };
    ErrorBody::other_version(
        format!(
            "the daemon is {}, not {BUILD} like this command, and {open}, so it was left running; {RESTART_ADVICE}",
            daemon_of(running),
        ),
        OtherVersion { cli_version: Some(BUILD.to_string()), daemon_version: running.map(str::to_string), terminals },
    )
}

fn connect() -> Result<Option<UnixStream>> {
    let socket = socket_path()?;
    match UnixStream::connect(&socket) {
        Ok(stream) => Ok(Some(stream)),
        Err(e) if is_not_running(&e) => Ok(None),
        Err(e) => Err(e).with_context(|| format!("connect to the daemon at {}", socket.display())),
    }
}

fn connect_or_start() -> Result<UnixStream> {
    let home = home()?;
    let _lock = start_lock(&home)?;
    match UnixStream::connect(socket_path()?) {
        Ok(stream) => Ok(stream),
        Err(e) if is_not_running(&e) => start_daemon(&home),
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

fn is_not_running(error: &std::io::Error) -> bool {
    matches!(error.kind(), ErrorKind::NotFound | ErrorKind::ConnectionRefused)
}

fn start_daemon(home: &std::path::Path) -> Result<UnixStream> {
    let log_path = home.join("daemon.log");
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("open {}", log_path.display()))?;
    keep_inherited_descriptors_from_the_daemon()?;
    let mut daemon = Command::new(std::env::current_exe()?)
        .args(["daemon", "run"])
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
            .with_context(|| format!("/dev/fd lists \"{}\", which is not a descriptor number", name.display()))?;
        if descriptor > 2 {
            fcntl(descriptor, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC))
                .with_context(|| format!("mark descriptor {descriptor} close-on-exec"))?;
        }
    }
    Ok(())
}
