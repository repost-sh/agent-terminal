use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, ensure};

use crate::daemon::{Agent, Daemon};
use crate::metrics::{Metric, Results, Samples};

const RUNS: usize = 100;
const MEMORY_RUNS: usize = 5;
const TABS: u32 = 10;
pub const SIZE: &str = "120x40";
const SCROLLBACK: &str = "2000";

pub struct Setup {
    pub base: PathBuf,
    pub change: PathBuf,
    pub fixtures: PathBuf,
    pub work: PathBuf,
}

const BASE: usize = 0;

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

    let mut idle = Sampled::default();
    let mut tab = Sampled::default();
    for run in 0..MEMORY_RUNS {
        for side in order(run) {
            match memory(agent(side, format!("{}-memory-{run}", binaries[side].0)), &busy_screen) {
                Ok((without_tabs, per_tab)) => {
                    idle.record(side, Ok(without_tabs))?;
                    tab.record(side, per_tab)?;
                }
                Err(e) => {
                    idle.record(side, Err(anyhow::anyhow!("{e:#}")))?;
                    tab.record(side, Err(e))?;
                }
            }
        }
    }

    let mut base = Screen::open(agent(0, binaries[0].0.to_string()), &busy_screen);
    let mut change = Screen::open(agent(1, binaries[1].0.to_string()), &busy_screen)?;
    let round_trip = interleave(&mut base, &mut change, |screen| screen.time(&["get", "frame"]))?;
    let snapshot = interleave(&mut base, &mut change, |screen| screen.time(&["snapshot"]))?;
    let png = interleave(&mut base, &mut change, |screen| screen.time(&["screenshot", &screen.screenshot]))?;
    let wait = interleave(&mut base, &mut change, |screen| screen.press_wait_after_text())?;
    if let Ok(screen) = &mut base {
        screen.daemon.stop()?;
    }
    change.daemon.stop()?;

    Ok(Results {
        metrics: vec![
            round_trip.samples(Metric::RoundTrip),
            snapshot.samples(Metric::Snapshot),
            png.samples(Metric::Png),
            wait.samples(Metric::PressWaitText),
            idle.samples(Metric::IdleMemory),
            tab.samples(Metric::TabMemory),
        ],
    })
}

fn memory(agent: Agent, busy_screen: &Path) -> Result<(f64, Result<f64>)> {
    let mut daemon = Daemon::start(agent)?;
    daemon.agent.run(&["session", "list"])?;
    let without_tabs = daemon.memory()?;
    let per_tab = (|| {
        for _ in 0..TABS {
            daemon.agent.run(&[
                "tab",
                "new",
                "--size",
                SIZE,
                "--wait",
                "ready",
                utf8(busy_screen)?,
                "--scrollback",
                SCROLLBACK,
            ])?;
        }
        Ok((daemon.memory()? - without_tabs) / f64::from(TABS))
    })();
    daemon.stop()?;
    Ok((without_tabs, per_tab))
}

fn order(run: usize) -> [usize; 2] {
    if run.is_multiple_of(2) { [0, 1] } else { [1, 0] }
}

#[derive(Default)]
struct Sampled {
    base: Vec<f64>,
    no_base: Option<String>,
    change: Vec<f64>,
}

impl Sampled {
    fn record(&mut self, side: usize, sample: Result<f64>) -> Result<()> {
        match (side, sample) {
            (BASE, _) if self.no_base.is_some() => {}
            (BASE, Ok(value)) => self.base.push(value),
            (BASE, Err(e)) => {
                self.base.clear();
                self.no_base = Some(first_line(&format!("{e:#}")));
            }
            (_, Ok(value)) => self.change.push(value),
            (_, Err(e)) => return Err(e),
        }
        Ok(())
    }

    fn samples(self, metric: Metric) -> Samples {
        Samples { metric, base: self.base, no_base: self.no_base, change: self.change }
    }
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or(text).to_string()
}

fn interleave(
    base: &mut Result<Screen>,
    change: &mut Screen,
    mut sample: impl FnMut(&mut Screen) -> Result<f64>,
) -> Result<Sampled> {
    let mut sampled = Sampled::default();
    for run in 0..RUNS {
        for side in order(run) {
            let taken = match (side, &mut *base) {
                (BASE, Ok(screen)) => sample(screen),
                (BASE, Err(e)) => Err(anyhow::anyhow!("{e:#}")),
                _ => sample(change),
            };
            sampled.record(side, taken)?;
        }
    }
    Ok(sampled)
}

fn canonical(path: &Path) -> Result<PathBuf> {
    std::fs::canonicalize(path).with_context(|| format!("find {}", path.display()))
}

pub fn utf8(path: &Path) -> Result<&str> {
    path.to_str().with_context(|| format!("{} is not UTF-8", path.display()))
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
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

    fn press_wait_after_text(&mut self) -> Result<f64> {
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
