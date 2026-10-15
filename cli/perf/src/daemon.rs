use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command, Output, Stdio};

use anyhow::{Context, Result, ensure};

pub struct Agent {
    pub side: &'static str,
    pub binary: PathBuf,
    pub work: PathBuf,
    pub home: String,
}

impl Agent {
    pub fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(&self.binary);
        command
            .args(args)
            .current_dir(&self.work)
            .env("AGENT_TERMINAL_HOME", &self.home)
            .env_remove("AGENT_TERMINAL_SESSION")
            .env_remove("AGENT_TERMINAL_SCREENSHOT_DIR")
            .stdin(Stdio::null());
        command
    }

    pub fn run(&self, args: &[&str]) -> Result<Output> {
        let output = self.command(args).output().with_context(|| format!("run {}", self.binary.display()))?;
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

    pub fn home(&self) -> PathBuf {
        self.work.join(&self.home)
    }
}

pub struct Daemon {
    pub agent: Agent,
    process: Child,
    _ready: BufReader<ChildStdout>,
}

impl Daemon {
    pub fn start(agent: Agent) -> Result<Daemon> {
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
        let daemon = Daemon { agent, process, _ready: ready };
        ensure!(line.trim() == "ready", "the {} daemon did not start: {line:?}", daemon.agent.side);
        Ok(daemon)
    }

    fn ps(&self, field: &str) -> Result<String> {
        let output =
            Command::new("ps").args(["-o", field, "-p", &self.process.id().to_string()]).output().context("run ps")?;
        ensure!(output.status.success(), "ps found no {} daemon: {}", self.agent.side, output.status);
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn memory(&self) -> Result<f64> {
        let printed = self.ps("rss=")?;
        let kibibytes: f64 =
            printed.parse().with_context(|| format!("read the resident size ps printed: {printed:?}"))?;
        Ok(kibibytes * 1024.0 / 1_000_000.0)
    }

    pub fn cpu_seconds(&self) -> Result<f64> {
        let time = self.ps("time=")?;
        let mut seconds = 0.0;
        for part in time.split(':') {
            seconds = seconds * 60.0 + part.parse::<f64>().with_context(|| format!("read the CPU time {time:?}"))?;
        }
        Ok(seconds)
    }

    pub fn fds(&self) -> Result<usize> {
        let output = Command::new("lsof")
            .args(["-n", "-P", "-p", &self.process.id().to_string()])
            .output()
            .context("run lsof")?;
        Ok(String::from_utf8_lossy(&output.stdout).lines().count().saturating_sub(1))
    }

    pub fn dashboard(&self) -> Result<u16> {
        let output = self.agent.run(&["--json", "dashboard", "start", "--port", "0"])?;
        let started: serde_json::Value = serde_json::from_slice(&output.stdout).context("read the dashboard")?;
        let port = started["port"].as_u64().context("the dashboard's port")?;
        Ok(u16::try_from(port)?)
    }

    pub fn stop(&mut self) -> Result<()> {
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
            eprintln!("stopping the {} daemon failed: {e:#}", self.agent.side);
            if let Err(e) = self.process.kill() {
                eprintln!("killing the {} daemon failed: {e}", self.agent.side);
            }
        }
    }
}
