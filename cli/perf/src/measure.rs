use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, ensure};

use crate::daemon::{Agent, Binary, Daemon, SESSION};
use crate::metrics::{Metric, Results, Samples};

const RUNS: usize = 100;
const MEMORY_RUNS: usize = 5;
const TABS: u32 = 10;
pub(crate) const SIZE: &str = "120x40";
const SCROLLBACK: &str = "2000";

pub(crate) struct Setup {
    pub base: PathBuf,
    pub change: PathBuf,
    pub fixtures: PathBuf,
    pub work: PathBuf,
}

#[derive(Clone, Copy)]
enum Side {
    Base,
    Change,
}

impl Side {
    fn name(self) -> &'static str {
        match self {
            Side::Base => "base",
            Side::Change => "change",
        }
    }
}

struct Sides {
    work: PathBuf,
    base: Binary,
    change: Binary,
}

impl Sides {
    fn agent(&self, side: Side, home: String) -> Agent {
        let binary = match side {
            Side::Base => &self.base,
            Side::Change => &self.change,
        };
        Agent { side: side.name(), binary: binary.clone(), work: self.work.clone(), home }
    }
}

struct Timings {
    round_trip: Sampled,
    snapshot: Sampled,
    png: Sampled,
    wait: Sampled,
}

pub(crate) fn measure(setup: &Setup) -> Result<Results> {
    std::fs::create_dir_all(&setup.work).with_context(|| format!("create {}", setup.work.display()))?;
    let work = canonical(&setup.work)?;
    let busy_screen = canonical(&setup.fixtures.join("busy-screen"))?;
    let sides = Sides {
        work,
        base: Binary::detect(canonical(&setup.base)?),
        change: Binary::detect(canonical(&setup.change)?),
    };
    let (idle, tab) = memory_runs(&sides, &busy_screen)?;
    let timings = timings(&sides, &busy_screen)?;
    Ok(Results {
        metrics: vec![
            timings.round_trip.samples(Metric::RoundTrip),
            timings.snapshot.samples(Metric::Snapshot),
            timings.png.samples(Metric::Png),
            timings.wait.samples(Metric::PressWaitText),
            idle.samples(Metric::IdleMemory),
            tab.samples(Metric::TabMemory),
        ],
    })
}

fn memory_runs(sides: &Sides, busy_screen: &Path) -> Result<(Sampled, Sampled)> {
    let mut idle = Sampled::default();
    let mut tab = Sampled::default();
    for run in 0..MEMORY_RUNS {
        for side in order(run) {
            match memory(sides.agent(side, format!("{}-memory-{run}", side.name())), busy_screen) {
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
    Ok((idle, tab))
}

fn timings(sides: &Sides, busy_screen: &Path) -> Result<Timings> {
    let mut base = Screen::open(sides.agent(Side::Base, Side::Base.name().to_string()), busy_screen);
    let mut change = Screen::open(sides.agent(Side::Change, Side::Change.name().to_string()), busy_screen)?;
    let timings = Timings {
        round_trip: interleave(&mut base, &mut change, |screen| screen.time(&["get", "frame"]))?,
        snapshot: interleave(&mut base, &mut change, |screen| screen.time(&["snapshot"]))?,
        png: interleave(&mut base, &mut change, |screen| screen.time(&["screenshot", &screen.screenshot]))?,
        wait: interleave(&mut base, &mut change, Screen::press_wait_after_text)?,
    };
    if let Ok(screen) = &mut base {
        screen.daemon.stop()?;
    }
    change.daemon.stop()?;
    Ok(timings)
}

fn memory(agent: Agent, busy_screen: &Path) -> Result<(f64, Result<f64>)> {
    let mut daemon = Daemon::start(agent)?;
    daemon.agent().run(&["session", "list"])?;
    daemon.agent().new_session(SESSION)?;
    let without_tabs = daemon.memory()?;
    let per_tab = (|| {
        for tab in 0..TABS {
            let args = ["--size", SIZE, "--wait", "ready", utf8(busy_screen)?, "--scrollback", SCROLLBACK];
            daemon.agent().open(&format!("tab-{tab}"), &args)?;
        }
        Ok((daemon.memory()? - without_tabs) / f64::from(TABS))
    })();
    daemon.stop()?;
    Ok((without_tabs, per_tab))
}

fn order(run: usize) -> [Side; 2] {
    if run.is_multiple_of(2) { [Side::Base, Side::Change] } else { [Side::Change, Side::Base] }
}

#[derive(Default)]
struct Sampled {
    base: Vec<f64>,
    no_base: Option<String>,
    change: Vec<f64>,
}

impl Sampled {
    fn record(&mut self, side: Side, sample: Result<f64>) -> Result<()> {
        match (side, sample) {
            (Side::Base, _) if self.no_base.is_some() => {}
            (Side::Base, Ok(value)) => self.base.push(value),
            (Side::Base, Err(e)) => {
                self.base.clear();
                self.no_base = Some(first_line(&format!("{e:#}")));
            }
            (Side::Change, Ok(value)) => self.change.push(value),
            (Side::Change, Err(e)) => return Err(e),
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
                (Side::Base, Ok(screen)) => sample(screen),
                (Side::Base, Err(e)) => Err(anyhow::anyhow!("{e:#}")),
                (Side::Change, _) => sample(change),
            };
            sampled.record(side, taken)?;
        }
    }
    Ok(sampled)
}

fn canonical(path: &Path) -> Result<PathBuf> {
    std::fs::canonicalize(path).with_context(|| format!("find {}", path.display()))
}

pub(crate) fn utf8(path: &Path) -> Result<&str> {
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
        daemon.agent().new_session(SESSION)?;
        daemon
            .agent()
            .open("screen", &["--size", SIZE, "--wait", "ready", utf8(busy_screen)?, "--stamps", utf8(&stamps)?])?;
        Ok(Screen { daemon, stamps, screenshot, pressed: 0 })
    }

    fn time(&self, args: &[&str]) -> Result<f64> {
        let started = Instant::now();
        self.daemon.agent().run(args)?;
        Ok(milliseconds(started.elapsed()))
    }

    fn press_wait_after_text(&mut self) -> Result<f64> {
        self.pressed += 1;
        self.daemon.agent().run(&["press", "x", "--wait", &format!("seen {}.", self.pressed)])?;
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
