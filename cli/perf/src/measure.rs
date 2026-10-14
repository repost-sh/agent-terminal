use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Output, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, ensure};

use crate::metrics::{Metric, Results, Samples};

const RUNS: usize = 100;
const MEMORY_RUNS: usize = 5;
const TABS: u32 = 10;
const SIZE: &str = "120x40";
const SCROLLBACK: &str = "2000";

pub struct Setup {
    pub base: PathBuf,
    pub change: PathBuf,
    pub fixtures: PathBuf,
    pub work: PathBuf,
}

pub fn measure(setup: &Setup) -> Result<Results> {
    std::fs::create_dir_all(&setup.work).with_context(|| format!("create {}", setup.work.display()))?;
    let work = canonical(&setup.work)?;
    let busy_screen = canonical(&setup.fixtures.join("busy-screen"))?;
    let binaries = [("base", canonical(&setup.base)?), ("change", canonical(&setup.change)?)];
    let agent = |side: usize, home: String| Agent {
        side: binaries[side].0,
        binary: binaries[side].1.clone(),
        work: work.clone(),
        home,
    };

    let mut idle = [Vec::new(), Vec::new()];
    let mut tab = [Vec::new(), Vec::new()];
    for run in 0..MEMORY_RUNS {
        for side in order(run) {
            let mut daemon = Daemon::start(agent(side, format!("{}-memory-{run}", binaries[side].0)))?;
            daemon.agent.run(&["session", "list"])?;
            let without_tabs = daemon.memory()?;
            for _ in 0..TABS {
                daemon.agent.run(&[
                    "tab",
                    "new",
                    "--size",
                    SIZE,
                    "--wait",
                    "ready",
                    utf8(&busy_screen)?,
                    "--scrollback",
                    SCROLLBACK,
                ])?;
            }
            let with_tabs = daemon.memory()?;
            daemon.stop()?;
            idle[side].push(without_tabs);
            tab[side].push((with_tabs - without_tabs) / f64::from(TABS));
        }
    }

    let mut base = Screen::open(agent(0, binaries[0].0.to_string()), &busy_screen)?;
    let mut change = Screen::open(agent(1, binaries[1].0.to_string()), &busy_screen)?;
    let mut screens = [&mut base, &mut change];
    let round_trip = interleave(&mut screens, |screen| screen.time(&["get", "frame"]))?;
    let snapshot = interleave(&mut screens, |screen| screen.time(&["snapshot"]))?;
    let png = interleave(&mut screens, |screen| screen.time(&["screenshot", &screen.screenshot]))?;
    let wait = interleave(&mut screens, |screen| screen.wait_after_text())?;
    for screen in screens {
        screen.daemon.stop()?;
    }

    Ok(Results {
        metrics: vec![
            samples(Metric::RoundTrip, round_trip),
            samples(Metric::Snapshot, snapshot),
            samples(Metric::Png, png),
            samples(Metric::WaitText, wait),
            samples(Metric::IdleMemory, idle),
            samples(Metric::TabMemory, tab),
        ],
    })
}

fn order(run: usize) -> [usize; 2] {
    if run.is_multiple_of(2) { [0, 1] } else { [1, 0] }
}

fn samples(metric: Metric, [base, change]: [Vec<f64>; 2]) -> Samples {
    Samples { metric, base, change }
}

fn interleave(
    screens: &mut [&mut Screen; 2],
    mut sample: impl FnMut(&mut Screen) -> Result<f64>,
) -> Result<[Vec<f64>; 2]> {
    let mut samples = [Vec::with_capacity(RUNS), Vec::with_capacity(RUNS)];
    for run in 0..RUNS {
        for side in order(run) {
            samples[side].push(sample(screens[side])?);
        }
    }
    Ok(samples)
}

fn canonical(path: &Path) -> Result<PathBuf> {
    std::fs::canonicalize(path).with_context(|| format!("find {}", path.display()))
}

fn utf8(path: &Path) -> Result<&str> {
    path.to_str().with_context(|| format!("{} is not UTF-8", path.display()))
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

struct Agent {
    side: &'static str,
    binary: PathBuf,
    work: PathBuf,
    home: String,
}

impl Agent {
    fn command(&self, args: &[&str]) -> Command {
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

    fn run(&self, args: &[&str]) -> Result<Output> {
        let output = self.command(args).output().with_context(|| format!("run {}", self.binary.display()))?;
        ensure!(
            output.status.success(),
            "`agent-terminal {}` failed for the {} ({}):\n{}{}",
            args.join(" "),
            self.side,
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(output)
    }
}

struct Daemon {
    agent: Agent,
    process: Child,
    _ready: BufReader<ChildStdout>,
}

impl Daemon {
    fn start(agent: Agent) -> Result<Daemon> {
        let home = agent.work.join(&agent.home);
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

    fn memory(&self) -> Result<f64> {
        let output =
            Command::new("ps").args(["-o", "rss=", "-p", &self.process.id().to_string()]).output().context("run ps")?;
        ensure!(output.status.success(), "ps found no {} daemon: {}", self.agent.side, output.status);
        let kibibytes: f64 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .with_context(|| format!("read the resident size ps printed: {:?}", output.stdout))?;
        Ok(kibibytes * 1024.0 / 1_000_000.0)
    }

    fn stop(&mut self) -> Result<()> {
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

struct Screen {
    daemon: Daemon,
    stamps: PathBuf,
    screenshot: String,
    pressed: u64,
}

impl Screen {
    fn open(agent: Agent, busy_screen: &Path) -> Result<Screen> {
        let stamps = agent.work.join(&agent.home).join("stamps");
        let screenshot = format!("{}.png", agent.home);
        let daemon = Daemon::start(agent)?;
        daemon.agent.run(&[
            "open",
            "--size",
            SIZE,
            "--wait",
            "ready",
            utf8(busy_screen)?,
            "--stamps",
            utf8(&stamps)?,
        ])?;
        Ok(Screen { daemon, stamps, screenshot, pressed: 0 })
    }

    fn time(&self, args: &[&str]) -> Result<f64> {
        let started = Instant::now();
        self.daemon.agent.run(args)?;
        Ok(milliseconds(started.elapsed()))
    }

    fn wait_after_text(&mut self) -> Result<f64> {
        self.pressed += 1;
        self.daemon.agent.run(&["press", "x", "--wait", &format!("seen {}.", self.pressed)])?;
        let returned = SystemTime::now();
        let printed = self.printed(self.pressed)?;
        Ok(milliseconds(returned.duration_since(printed).context("the wait returned before the text was printed")?))
    }

    fn printed(&self, seen: u64) -> Result<SystemTime> {
        let stamps =
            std::fs::read_to_string(&self.stamps).with_context(|| format!("read {}", self.stamps.display()))?;
        let last = stamps.lines().last().context("busy-screen wrote no stamp")?;
        let (count, nanos) = last.split_once(' ').with_context(|| format!("read the stamp {last:?}"))?;
        ensure!(count == seen.to_string(), "busy-screen stamped press {count}, not {seen}");
        let nanos: u64 = nanos.parse().with_context(|| format!("read the stamp {last:?}"))?;
        Ok(UNIX_EPOCH + Duration::from_nanos(nanos))
    }
}
