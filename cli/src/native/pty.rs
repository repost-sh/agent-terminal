use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use portable_pty::{ChildKiller, CommandBuilder, PtySize, native_pty_system};

use crate::protocol::Size;

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
    pub wait: Box<dyn FnOnce() -> Result<i32> + Send>,
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

    let pair = native_pty_system()
        .openpty(PtySize { rows: command.size.rows, cols: command.size.cols, pixel_width: 0, pixel_height: 0 })
        .context("open a pseudo-terminal")?;
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
    let master = pair.master;
    Ok(Spawned {
        reader,
        writer,
        killer: Box::new(killer),
        wait: Box::new(move || {
            let status = child.wait().context("wait for the program to exit");
            drop(master);
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
