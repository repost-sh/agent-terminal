use std::fs::File;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::native::pty;
use crate::protocol::Shell;

struct Script {
    shell: Shell,
    path: &'static str,
    text: &'static str,
}

const SCRIPTS: [Script; 4] = [
    Script { shell: Shell::Zsh, path: "zsh/.zshenv", text: include_str!("zshenv.zsh") },
    Script { shell: Shell::Zsh, path: "zsh/agent-terminal.zsh", text: include_str!("agent-terminal.zsh") },
    Script { shell: Shell::Bash, path: "bash/agent-terminal.bash", text: include_str!("agent-terminal.bash") },
    Script { shell: Shell::Fish, path: "fish/agent-terminal.fish", text: include_str!("agent-terminal.fish") },
];

pub(crate) struct Scripts<'a> {
    pub home: &'a Path,
}

impl Scripts<'_> {
    fn folder(&self) -> Result<PathBuf> {
        let folder = self.home.join("shell");
        std::path::absolute(&folder).with_context(|| format!("find where {} is", folder.display()))
    }

    pub(crate) fn prepare(&self, shell: Shell, mut command: pty::Command) -> Result<pty::Command> {
        let folder = self.folder()?;
        {
            let _lock = self.lock()?;
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

    fn lock(&self) -> Result<File> {
        std::fs::create_dir_all(self.home).with_context(|| format!("create {}", self.home.display()))?;
        let path = self.home.join("shell.lock");
        let lock = File::create(&path).with_context(|| format!("create {}", path.display()))?;
        lock.lock().with_context(|| format!("lock {}", path.display()))?;
        Ok(lock)
    }
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

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn fish_quoted(text: &str) -> String {
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
}

#[cfg(test)]
mod tests;
