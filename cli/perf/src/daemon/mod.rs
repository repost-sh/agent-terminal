use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command, Output, Stdio};

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;

use crate::log;

pub(crate) const SESSION: &str = "perf";

const FIRST_UNNAMED_TAB: &str = "t1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dialect {
    Unnamed,
    Named,
}

#[derive(Clone)]
pub(crate) struct Binary {
    pub path: PathBuf,
    pub dialect: Dialect,
}

impl Binary {
    pub(crate) fn detect(path: PathBuf) -> Binary {
        let named = Command::new(&path)
            .args(["session", "new", "--help"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        Binary { path, dialect: if named { Dialect::Named } else { Dialect::Unnamed } }
    }
}

pub(crate) struct Agent {
    pub side: &'static str,
    pub binary: Binary,
    pub work: PathBuf,
    pub home: String,
}

impl Agent {
    pub(crate) fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(&self.binary.path);
        command
            .args(args)
            .current_dir(&self.work)
            .env("AGENT_TERMINAL_HOME", &self.home)
            .env("AGENT_TERMINAL_SESSION", SESSION)
            .env_remove("AGENT_TERMINAL_SCREENSHOT_DIR")
            .stdin(Stdio::null());
        command
    }

    pub(crate) fn run(&self, args: &[&str]) -> Result<Output> {
        let output = self.command(args).output().with_context(|| format!("run {}", self.binary.path.display()))?;
        ensure!(
            output.status.success(),
            "`agent-terminal {}` failed for the {} ({}): {}{}",
            args.join(" "),
            self.side,
            output.status,
            String::from_utf8_lossy(&output.stderr).trim_start(),
            String::from_utf8_lossy(&output.stdout)
        );
        Ok(output)
    }

    pub(crate) fn new_session(&self, name: &str) -> Result<()> {
        if self.binary.dialect == Dialect::Named {
            self.run(&["session", "new", name])?;
        }
        Ok(())
    }

    pub(crate) fn open(&self, tab: &str, args: &[&str]) -> Result<String> {
        match self.binary.dialect {
            Dialect::Named => {
                self.run(&[&["open", "--name", tab][..], args].concat())?;
                Ok(tab.to_string())
            }
            Dialect::Unnamed => {
                self.run(&[&["open"][..], args].concat())?;
                Ok(FIRST_UNNAMED_TAB.to_string())
            }
        }
    }

    pub(crate) fn home(&self) -> PathBuf {
        self.work.join(&self.home)
    }
}

pub(crate) struct Daemon {
    agent: Agent,
    process: Child,
    _ready: BufReader<ChildStdout>,
}

impl Daemon {
    pub(crate) fn start(agent: Agent) -> Result<Daemon> {
        let home = agent.home();
        std::fs::create_dir(&home)
            .with_context(|| format!("create {}; measure into an empty folder", home.display()))?;
        let log = std::fs::File::create(home.join("daemon.log")).context("create the daemon log")?;
        let mut process = agent
            .command(&["daemon", "run"])
            .stdout(Stdio::piped())
            .stderr(log)
            .spawn()
            .with_context(|| format!("start the {} daemon", agent.side))?;
        let mut ready = BufReader::new(process.stdout.take().context("daemon stdout")?);
        let mut line = String::new();
        ready.read_line(&mut line).with_context(|| format!("wait for the {} daemon", agent.side))?;
        if line.trim() != "ready" {
            process.kill().with_context(|| format!("end the {} program that did not start a daemon", agent.side))?;
            process
                .wait()
                .with_context(|| format!("wait for the {} program that did not start a daemon", agent.side))?;
            bail!("the {} daemon did not start: {line:?}", agent.side);
        }
        Ok(Daemon { agent, process, _ready: ready })
    }

    pub(crate) fn agent(&self) -> &Agent {
        &self.agent
    }

    fn ps(&self, field: &str) -> Result<String> {
        let output =
            Command::new("ps").args(["-o", field, "-p", &self.process.id().to_string()]).output().context("run ps")?;
        ensure!(output.status.success(), "ps found no {} daemon: {}", self.agent.side, output.status);
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub(crate) fn memory(&self) -> Result<f64> {
        let printed = self.ps("rss=")?;
        let kibibytes: f64 =
            printed.parse().with_context(|| format!("read the resident size ps printed: {printed:?}"))?;
        Ok(kibibytes * 1024.0 / 1_000_000.0)
    }

    pub(crate) fn cpu_seconds(&self) -> Result<f64> {
        let time = self.ps("time=")?;
        let mut seconds = 0.0;
        for part in time.split(':') {
            seconds = seconds * 60.0 + part.parse::<f64>().with_context(|| format!("read the CPU time {time:?}"))?;
        }
        Ok(seconds)
    }

    pub(crate) fn fds(&self) -> Result<usize> {
        let output = Command::new("lsof")
            .args(["-n", "-P", "-p", &self.process.id().to_string()])
            .output()
            .context("run lsof")?;
        Ok(String::from_utf8_lossy(&output.stdout).lines().count().saturating_sub(1))
    }

    pub(crate) fn dashboard(&self) -> Result<u16> {
        let output = self.agent.run(&["--json", "dashboard", "start", "--port", "0"])?;
        let started: Value = serde_json::from_slice(&output.stdout).context("read the dashboard")?;
        let port = started.get("port").and_then(Value::as_u64).context("the dashboard's port")?;
        Ok(u16::try_from(port)?)
    }

    pub(crate) fn stop(&mut self) -> Result<()> {
        if self.process.try_wait().context("check the daemon")?.is_some() {
            return Ok(());
        }
        self.agent.run(&["daemon", "stop"])?;
        let status = self.process.wait().with_context(|| format!("wait for the {} daemon to stop", self.agent.side))?;
        ensure!(status.success(), "the {} daemon exited with {status}", self.agent.side);
        Ok(())
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if let Err(e) = self.stop() {
            log::note(format_args!("stopping the {} daemon failed: {e:#}", self.agent.side));
            if let Err(e) = self.process.kill() {
                log::note(format_args!("killing the {} daemon failed: {e}", self.agent.side));
            }
        }
    }
}

#[cfg(test)]
mod tests;
