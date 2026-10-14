use std::fs::File;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::connection;
use crate::native::pty;
use crate::protocol::Shell;

struct Script {
    shell: Shell,
    path: &'static str,
    text: &'static str,
}

const SCRIPTS: [Script; 4] = [
    Script { shell: Shell::Zsh, path: "zsh/.zshenv", text: include_str!("shell/zshenv.zsh") },
    Script { shell: Shell::Zsh, path: "zsh/agent-terminal.zsh", text: include_str!("shell/agent-terminal.zsh") },
    Script { shell: Shell::Bash, path: "bash/agent-terminal.bash", text: include_str!("shell/agent-terminal.bash") },
    Script { shell: Shell::Fish, path: "fish/agent-terminal.fish", text: include_str!("shell/agent-terminal.fish") },
];

pub struct Audit {
    pub folder: PathBuf,
    pub current: usize,
    pub outdated: Vec<PathBuf>,
    pub leftovers: Vec<PathBuf>,
}

pub fn folder() -> Result<PathBuf> {
    let folder = connection::home()?.join("shell");
    std::path::absolute(&folder).with_context(|| format!("find where {} is", folder.display()))
}

pub fn prepare(shell: Shell, mut command: pty::Command) -> Result<pty::Command> {
    let folder = folder()?;
    {
        let _lock = lock()?;
        for script in SCRIPTS.iter().filter(|script| script.shell == shell) {
            write_if_changed(&folder, script)?;
        }
    }
    let dir = folder.join(shell.name());
    command.args = match shell {
        Shell::Zsh => {
            let user = command.env.iter().find(|(key, _)| key == "ZDOTDIR").map(|(_, value)| value.clone());
            command.env.retain(|(key, _)| key != "ZDOTDIR" && key != "AGENT_TERMINAL_ZDOTDIR");
            command.env.extend(user.map(|value| ("AGENT_TERMINAL_ZDOTDIR".to_string(), value)));
            command.env.push(("ZDOTDIR".to_string(), text(&dir)));
            vec!["-i".to_string()]
        }
        Shell::Bash => vec!["--rcfile".to_string(), text(&dir.join("agent-terminal.bash")), "-i".to_string()],
        Shell::Fish => vec![
            "--interactive".to_string(),
            "--features".to_string(),
            "no-mark-prompt".to_string(),
            "--init-command".to_string(),
            format!("source {}", fish_quoted(&text(&dir.join("agent-terminal.fish")))),
        ],
    };
    Ok(command)
}

pub fn audit() -> Result<Audit> {
    let folder = folder()?;
    let mut audit = Audit { folder: folder.clone(), current: 0, outdated: Vec::new(), leftovers: Vec::new() };
    for entry in entries(&folder)? {
        let name = entry.file_name().and_then(|name| name.to_str());
        if !Shell::ALL.iter().any(|shell| name == Some(shell.name())) || !entry.is_dir() {
            audit.leftovers.push(entry);
            continue;
        }
        for file in entries(&entry)? {
            match SCRIPTS.iter().find(|script| folder.join(script.path) == file) {
                Some(script) if read(&file)?.as_deref() == Some(script.text) => audit.current += 1,
                Some(_) => audit.outdated.push(file),
                None => audit.leftovers.push(file),
            }
        }
    }
    Ok(audit)
}

pub fn repair() -> Result<Audit> {
    let _lock = lock()?;
    let found = audit()?;
    for leftover in &found.leftovers {
        let removed =
            if leftover.is_dir() { std::fs::remove_dir_all(leftover) } else { std::fs::remove_file(leftover) };
        removed.with_context(|| format!("remove {}", leftover.display()))?;
    }
    for outdated in &found.outdated {
        let script = SCRIPTS
            .iter()
            .find(|script| found.folder.join(script.path) == *outdated)
            .with_context(|| format!("{} is not a shell integration script", outdated.display()))?;
        write_if_changed(&found.folder, script)?;
    }
    Ok(found)
}

fn lock() -> Result<File> {
    let home = connection::home()?;
    std::fs::create_dir_all(&home).with_context(|| format!("create {}", home.display()))?;
    let path = home.join("shell.lock");
    let lock = File::create(&path).with_context(|| format!("create {}", path.display()))?;
    lock.lock().with_context(|| format!("lock {}", path.display()))?;
    Ok(lock)
}

fn write_if_changed(folder: &Path, script: &Script) -> Result<()> {
    let path = folder.join(script.path);
    if read(&path)?.as_deref() == Some(script.text) {
        return Ok(());
    }
    let dir = path.parent().with_context(|| format!("{} has no folder", path.display()))?;
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let temporary = path.with_extension("partial");
    std::fs::write(&temporary, script.text).with_context(|| format!("write {}", temporary.display()))?;
    std::fs::rename(&temporary, &path).with_context(|| format!("write {}", path.display()))
}

fn read(path: &Path) -> Result<Option<String>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("read {}", path.display())),
    }
}

fn entries(dir: &Path) -> Result<Vec<PathBuf>> {
    let listing = match std::fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("list {}", dir.display())),
    };
    let mut paths = Vec::new();
    for entry in listing {
        paths.push(entry.with_context(|| format!("list {}", dir.display()))?.path());
    }
    paths.sort();
    Ok(paths)
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn fish_quoted(text: &str) -> String {
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
}
