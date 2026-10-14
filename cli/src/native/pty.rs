use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow};
use nix::sys::signal::{Signal as NativeSignal, killpg};
use nix::unistd::Pid;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::protocol::{Signal, Size};

#[derive(Clone)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub size: Size,
}

pub struct Spawned {
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    pub killer: Box<dyn Killer>,
    pub master: Master,
    pub pid: Option<u32>,
    pub signaller: Box<dyn Signaller>,
    pub wait: Box<dyn FnOnce() -> Result<i32> + Send>,
}

type SharedMaster = Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>;

pub struct Master(SharedMaster);

impl Master {
    pub fn resize(&self, size: Size) -> Result<bool> {
        let master = self.0.lock().unwrap();
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

pub trait Killer: Send {
    fn kill(&mut self) -> std::io::Result<()>;
}

pub trait Signaller: Send + Sync {
    fn signal(&self, signal: Signal) -> Result<i32>;
}

struct PortableKiller(Box<dyn ChildKiller + Send + Sync>);

impl Killer for PortableKiller {
    fn kill(&mut self) -> std::io::Result<()> {
        self.0.kill()
    }
}

struct ForegroundSignaller(SharedMaster);

impl Signaller for ForegroundSignaller {
    fn signal(&self, signal: Signal) -> Result<i32> {
        let group = self
            .0
            .lock()
            .unwrap()
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

pub fn spawn(command: Command) -> Result<Spawned> {
    let path_var = command.env.iter().find(|(k, _)| k == "PATH").map(|(_, v)| v.as_str()).unwrap_or("");
    let program = resolve(&command.program, &command.cwd, path_var)?;

    let pair = native_pty_system().openpty(pty_size(command.size)).context("open a pseudo-terminal")?;
    let mut builder = CommandBuilder::new(program);
    builder.args(&command.args);
    builder.cwd(&command.cwd);
    builder.env_clear();
    for (key, value) in &command.env {
        builder.env(key, value);
    }
    builder.env("TERM", "xterm-256color");
    builder.env("COLORTERM", "truecolor");

    let mut child = pair.slave.spawn_command(builder).with_context(|| format!("start {}", command.program))?;
    drop(pair.slave);
    let killer = PortableKiller(child.clone_killer());
    let reader = pair.master.try_clone_reader().context("read from the pseudo-terminal")?;
    let writer = pair.master.take_writer().context("write to the pseudo-terminal")?;
    let master: SharedMaster = Arc::new(Mutex::new(Some(pair.master)));
    Ok(Spawned {
        reader,
        writer,
        killer: Box::new(killer),
        master: Master(Arc::clone(&master)),
        pid: child.process_id(),
        signaller: Box::new(ForegroundSignaller(Arc::clone(&master))),
        wait: Box::new(move || {
            let status = child.wait().context("wait for the program to exit");
            drop(master.lock().unwrap().take());
            Ok(status?.exit_code() as i32)
        }),
    })
}

fn resolve(program: &str, cwd: &Path, path_var: &str) -> Result<PathBuf> {
    let found = if program.contains('/') {
        Some(cwd.join(program)).filter(|path| is_executable(path))
    } else {
        std::env::split_paths(path_var).map(|dir| dir.join(program)).find(|candidate| is_executable(candidate))
    };
    found.ok_or_else(|| crate::Error::new(crate::exit::ERROR, format!("command not found: {program}")).into())
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
