use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

use crate::programs;

pub(crate) struct Ran {
    pub(crate) code: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

#[derive(Default)]
pub(crate) struct Terminal {
    home: Option<TempDir>,
    pub(crate) ran: Option<Ran>,
    pub(crate) failure: Option<String>,
}

impl Terminal {
    fn home(&mut self) -> Result<&Path, String> {
        if self.home.is_none() {
            self.home = Some(tempfile::tempdir().map_err(|e| format!("make a home for agent-terminal: {e}"))?);
        }
        self.home.as_ref().map(TempDir::path).ok_or_else(|| "the home was just made".to_string())
    }

    pub(crate) fn run(&mut self, args: &[&str]) -> Result<Ran, String> {
        let home = self.home()?.to_path_buf();
        let binary = programs::agent_terminal()?;
        let output = Command::new(binary)
            .args(args)
            .env("AGENT_TERMINAL_HOME", &home)
            .env_remove("AGENT_TERMINAL_SESSION")
            .output()
            .map_err(|e| format!("run {}: {e}", binary.display()))?;
        Ok(Ran {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    pub(crate) fn setup(&mut self, args: &[&str]) {
        match self.run(args) {
            Ok(Ran { code: Some(0), .. }) => {}
            Ok(ran) => self.fail(format!("`agent-terminal {}` failed: {}", args.join(" "), ran.stderr)),
            Err(failure) => self.fail(failure),
        }
    }

    pub(crate) fn act(&mut self, args: &[&str]) {
        match self.run(args) {
            Ok(ran) => self.ran = Some(ran),
            Err(failure) => self.fail(failure),
        }
    }

    fn fail(&mut self, failure: String) {
        self.failure.get_or_insert(failure);
    }

    pub(crate) fn ran(&self) -> Option<&Ran> {
        self.ran.as_ref().filter(|_| self.failure.is_none())
    }

    pub(crate) fn report(&self) -> String {
        match (&self.failure, &self.ran) {
            (Some(failure), _) => format!("a step before this one failed: {failure}"),
            (None, Some(ran)) => format!("the last command printed {:?} and {:?}", ran.stdout, ran.stderr),
            (None, None) => "no command ran before this step".to_string(),
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if self.home.is_some() {
            drop(self.run(&["daemon", "stop"]));
        }
    }
}
