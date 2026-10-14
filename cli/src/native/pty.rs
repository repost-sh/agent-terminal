use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::protocol::Size;

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
    pub wait: Box<dyn FnOnce() -> Result<i32> + Send>,
}

#[derive(Clone)]
pub struct Master(Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>);

impl Master {
    pub fn resize(&self, size: Size) -> Result<bool> {
        let master = self.0.lock().unwrap();
        let Some(master) = master.as_ref() else {
            return Ok(false);
        };
        master.resize(pty_size(size)).context("resize the pseudo-terminal")?;
        Ok(true)
    }

    fn close(&self) {
        self.0.lock().unwrap().take();
    }
}

fn pty_size(size: Size) -> PtySize {
    PtySize { rows: size.rows, cols: size.cols, pixel_width: 0, pixel_height: 0 }
}

pub trait Killer: Send {
    fn kill(&mut self) -> std::io::Result<()>;
}

struct PortableKiller(Box<dyn ChildKiller + Send + Sync>);

impl Killer for PortableKiller {
    fn kill(&mut self) -> std::io::Result<()> {
        self.0.kill()
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
    let master = Master(Arc::new(Mutex::new(Some(pair.master))));
    let closing = master.clone();
    Ok(Spawned {
        reader,
        writer,
        killer: Box::new(killer),
        master,
        wait: Box::new(move || {
            let status = child.wait().context("wait for the program to exit");
            closing.close();
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
