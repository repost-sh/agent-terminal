use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use anyhow::{Context, Result, anyhow};
use nix::sys::signal::{Signal as NativeSignal, kill, killpg};
use nix::unistd::Pid;
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::failure::Failure;
use crate::protocol::{Signal, Size};

#[derive(Clone)]
pub(crate) struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub size: Size,
}

impl Command {
    pub(crate) fn line(&self) -> String {
        let typed: Vec<String> = std::iter::once(&self.program).chain(&self.args).cloned().collect();
        crate::output::call_line(&typed)
    }
}

pub(crate) struct Spawned {
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    pub stopper: Stopper,
    pub master: Master,
    pub pid: Option<u32>,
    pub signaller: Box<dyn Signaller>,
    pub wait: Box<dyn FnOnce() -> Result<i32> + Send>,
}

type SharedMaster = Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>;

pub(crate) struct Master(SharedMaster);

impl Master {
    pub(crate) fn resize(&self, size: Size) -> Result<bool> {
        let master = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(master) = master.as_ref() else {
            return Ok(false);
        };
        master.resize(pty_size(size)).context("resize the pseudo-terminal")?;
        Ok(true)
    }
}

fn pty_size(size: Size) -> PtySize {
    PtySize { rows: size.rows, cols: size.cols, pixel_width: 0, pixel_height: 0 }
}

pub(crate) trait Signaller: Send + Sync {
    fn signal(&self, signal: Signal) -> Result<i32>;
}

pub(crate) struct Stopper {
    pid: Option<u32>,
}

impl Stopper {
    pub(crate) fn hang_up(&self) -> Result<()> {
        self.signal(NativeSignal::SIGHUP)
    }

    pub(crate) fn kill(&self) -> Result<()> {
        self.signal(NativeSignal::SIGKILL)
    }

    fn signal(&self, signal: NativeSignal) -> Result<()> {
        let pid = self.pid.ok_or_else(|| anyhow!("the program has no process id to signal"))?;
        let pid = Pid::from_raw(i32::try_from(pid).context("a process id")?);
        match killpg(pid, signal) {
            Err(nix::errno::Errno::ESRCH) => kill(pid, signal),
            sent => sent,
        }
        .with_context(|| format!("send {signal} to process group {pid}"))
    }
}

struct ForegroundSignaller(SharedMaster);

impl Signaller for ForegroundSignaller {
    fn signal(&self, signal: Signal) -> Result<i32> {
        let group = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .and_then(|master| master.process_group_leader())
            .ok_or_else(|| anyhow!("the terminal has no foreground process to signal"))?;
        killpg(Pid::from_raw(group), native(signal))
            .with_context(|| format!("send {signal} to process group {group}"))?;
        Ok(group)
    }
}

fn native(signal: Signal) -> NativeSignal {
    match signal {
        Signal::Int => NativeSignal::SIGINT,
        Signal::Term => NativeSignal::SIGTERM,
        Signal::Kill => NativeSignal::SIGKILL,
        Signal::Hup => NativeSignal::SIGHUP,
        Signal::Tstp => NativeSignal::SIGTSTP,
        Signal::Cont => NativeSignal::SIGCONT,
        Signal::Winch => NativeSignal::SIGWINCH,
    }
}

pub(crate) fn spawn(command: &Command) -> Result<Spawned> {
    if !command.cwd.is_dir() {
        let missing = format!("the working directory {} does not exist", command.cwd.display());
        return Err(Failure::new(crate::exit::ERROR, missing).into());
    }
    let path_var = command.env.iter().find(|(k, _)| k == "PATH").map_or("", |(_, v)| v.as_str());
    let program = resolve(&command.program, &command.cwd, path_var)?;

    let pair = native_pty_system().openpty(pty_size(command.size)).context("open a pseudo-terminal")?;
    let mut builder = CommandBuilder::new(program);
    builder.args(&command.args);
    builder.cwd(&command.cwd);
    builder.env_clear();
    for (key, value) in environment(command) {
        builder.env(key, value);
    }

    let mut child = pair.slave.spawn_command(builder).with_context(|| format!("start {}", command.program))?;
    drop(pair.slave);
    let reader = pair.master.try_clone_reader().context("read from the pseudo-terminal")?;
    let writer = pair.master.take_writer().context("write to the pseudo-terminal")?;
    let master: SharedMaster = Arc::new(Mutex::new(Some(pair.master)));
    Ok(Spawned {
        reader,
        writer,
        stopper: Stopper { pid: child.process_id() },
        master: Master(Arc::clone(&master)),
        pid: child.process_id(),
        signaller: Box::new(ForegroundSignaller(Arc::clone(&master))),
        wait: Box::new(move || {
            let status = child.wait().context("wait for the program to exit");
            drop(master.lock().unwrap_or_else(PoisonError::into_inner).take());
            i32::try_from(status?.exit_code()).context("the program's exit code")
        }),
    })
}

const EMULATED: [(&str, &str); 2] = [("TERM", "xterm-256color"), ("COLORTERM", "truecolor")];

pub(crate) fn inherited(vars: impl IntoIterator<Item = (String, String)>) -> Vec<(String, String)> {
    vars.into_iter().filter(|(key, _)| !EMULATED.iter().any(|(name, _)| name == key)).collect()
}

pub(crate) fn environment(command: &Command) -> Vec<(String, String)> {
    let mut env = command.env.clone();
    for (name, value) in EMULATED {
        if !env.iter().any(|(key, _)| key == name) {
            env.push((name.to_string(), value.to_string()));
        }
    }
    env
}

pub(crate) fn check() -> Result<()> {
    native_pty_system()
        .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
        .map(drop)
        .context("open a pseudo-terminal")
}

fn resolve(program: &str, cwd: &Path, path_var: &str) -> Result<PathBuf> {
    let found = if program.contains('/') {
        Some(cwd.join(program)).filter(|path| is_executable(path))
    } else {
        std::env::split_paths(path_var).map(|dir| dir.join(program)).find(|candidate| is_executable(candidate))
    };
    found.ok_or_else(|| Failure::new(crate::exit::ERROR, format!("command not found: {program}")).into())
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
