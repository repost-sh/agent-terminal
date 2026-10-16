use std::path::{Path, PathBuf};

use crate::support::{AgentTerminal, Run};

pub(super) struct ShellTab {
    pub(super) at: AgentTerminal,
    shell: &'static str,
    pub(super) home: tempfile::TempDir,
    pub(super) work: tempfile::TempDir,
}

impl ShellTab {
    pub(super) fn new(shell: &'static str) -> Self {
        assert!(installed(shell), "the shell tests run in zsh, bash and fish, and {shell} is not on PATH; install it");
        let scratch = crate::support::scratch;
        Self { at: AgentTerminal::new(), shell, home: scratch("home-"), work: scratch("work-") }
    }

    pub(super) fn opened(shell: &'static str) -> Self {
        let tab = Self::new(shell);
        tab.open().success();
        tab
    }

    pub(super) fn open(&self) -> Run {
        let work = self.work.path().to_string_lossy().into_owned();
        self.run(&["shell", "--shell", self.shell, "--size", "60x10", "--cwd", &work])
    }

    pub(super) fn run(&self, args: &[&str]) -> Run {
        self.run_with(args, &[])
    }

    pub(super) fn run_with(&self, args: &[&str], env: &[(&str, Option<&str>)]) -> Run {
        let mut command = self.at.command(args);
        command.env("HOME", self.home.path()).env_remove("ZDOTDIR").env_remove("XDG_CONFIG_HOME");
        for (key, value) in env {
            match value {
                Some(value) => command.env(key, value),
                None => command.env_remove(key),
            };
        }
        Run(command.output().expect("run agent-terminal"))
    }

    pub(super) fn cwd(&self) -> String {
        canonical(self.work.path())
    }

    pub(super) fn hold(&self) -> PathBuf {
        let hold = self.work.path().join("hold");
        let status = std::process::Command::new("mkfifo").arg(&hold).status().expect("run mkfifo");
        assert!(status.success(), "mkfifo failed");
        hold
    }
}

fn canonical(path: &Path) -> String {
    std::fs::canonicalize(path).expect("canonical path").to_string_lossy().into_owned()
}

fn installed(shell: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir: PathBuf| dir.join(shell).is_file()))
}

pub(super) fn which(program: &str) -> String {
    let path = std::env::var_os("PATH").expect("PATH");
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| panic!("{program} is not on PATH"))
        .to_string_lossy()
        .into_owned()
}

pub(super) fn timeless(text: &str) -> String {
    let duration = regex::Regex::new(r" · (\d+ ms|\d+\.\d s|\d+ min \d{2} s) · ").unwrap();
    duration.replace_all(text, " · <time> · ").into_owned()
}
