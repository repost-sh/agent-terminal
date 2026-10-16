use std::fs::{File, OpenOptions, TryLockError};
use std::io::{ErrorKind, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::home::{DAEMON_LOCK, DAEMON_VERSION, SOCKET, running_build};
use crate::version::{BUILD, daemon_of};

pub(super) fn claim(home: &Path) -> Result<(File, UnixListener)> {
    let path = home.join(DAEMON_LOCK);
    let mut lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("open {}", path.display()))?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return another_daemon(home, &holder(&path)?),
        Err(TryLockError::Error(e)) => return Err(e).with_context(|| format!("lock {}", path.display())),
    }
    let listener = listen(home)?;
    lock.set_len(0)
        .and_then(|()| write!(lock, "{}", std::process::id()))
        .with_context(|| format!("write this daemon's pid to {}", path.display()))?;
    record_build(home)?;
    Ok((lock, listener))
}

fn holder(lock: &Path) -> Result<String> {
    let pid = std::fs::read_to_string(lock).with_context(|| format!("read the pid in {}", lock.display()))?;
    Ok(match pid.trim() {
        "" => "unknown".to_string(),
        pid => pid.to_string(),
    })
}

fn another_daemon<T>(home: &Path, pid: &str) -> Result<T> {
    let daemon = daemon_of(running_build(home)?.as_deref());
    bail!(
        "another agent-terminal daemon runs for this user: pid {pid}, {daemon}, listening on {}",
        home.join(SOCKET).display()
    )
}

fn listen(home: &Path) -> Result<UnixListener> {
    let path = home.join(SOCKET);
    match UnixListener::bind(&path) {
        Ok(listener) => Ok(listener),
        Err(e) if e.kind() == ErrorKind::AddrInUse && UnixStream::connect(&path).is_ok() => {
            another_daemon(home, "unknown")
        }
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            std::fs::remove_file(&path).with_context(|| format!("remove the stale socket {}", path.display()))?;
            UnixListener::bind(&path).with_context(|| format!("listen on {}", path.display()))
        }
        Err(e) => Err(e).with_context(|| format!("listen on {}", path.display())),
    }
}

fn record_build(home: &Path) -> Result<()> {
    let path = home.join(DAEMON_VERSION);
    let written = home.join("daemon.version.new");
    std::fs::write(&written, BUILD).with_context(|| format!("write {}", written.display()))?;
    std::fs::rename(&written, &path).with_context(|| format!("move {} to {}", written.display(), path.display()))
}
