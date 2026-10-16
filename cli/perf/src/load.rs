use std::fmt;
use std::io::Write;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use tungstenite::{Message, WebSocket};

use crate::daemon::{Agent, Daemon};
use crate::measure::{SIZE, utf8};

const SETTLE_SECONDS: u64 = 2;

pub(crate) struct Plan {
    pub agent: PathBuf,
    pub fixtures: PathBuf,
    pub work: PathBuf,
    pub seconds: u64,
    pub rates: Vec<u32>,
    pub tabs: Vec<usize>,
    pub repaints: Vec<Repaint>,
    pub viewers: Vec<Viewer>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Repaint {
    Partial,
    Full,
}

impl Repaint {
    const ALL: [Self; 2] = [Self::Partial, Self::Full];

    fn name(self) -> &'static str {
        match self {
            Self::Partial => "partial",
            Self::Full => "full",
        }
    }

    pub(crate) fn parse(text: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|repaint| repaint.name() == text)
            .with_context(|| format!("{text} is not a repaint; use {}", Self::ALL.map(Self::name).join(" or ")))
    }

    fn flag(self) -> String {
        format!("--{}", self.name())
    }
}

#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Viewer {
    None,
    Dashboard,
}

impl Viewer {
    pub(crate) const ALL: [Self; 2] = [Self::None, Self::Dashboard];

    fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Dashboard => "dashboard",
        }
    }

    pub(crate) fn parse(text: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|viewer| viewer.name() == text)
            .with_context(|| format!("{text} is not a viewer; use {}", Self::ALL.map(Self::name).join(" or ")))
    }
}

#[derive(Serialize)]
pub(crate) struct Row {
    pub hz: u32,
    pub repaint: Repaint,
    pub tabs: usize,
    pub viewer: Viewer,
    pub seconds: f64,
    pub program_bytes_per_sec: f64,
    pub program_frames_per_sec: f64,
    pub daemon_frames_per_sec: f64,
    pub daemon_cpu_percent: f64,
    pub daemon_rss_mb: f64,
    pub daemon_fds: usize,
    pub stream_messages_per_sec: f64,
    pub stream_bytes_per_sec: f64,
}

#[derive(Clone, Copy)]
struct Case {
    hz: u32,
    repaint: Repaint,
    tabs: usize,
    viewer: Viewer,
}

impl fmt::Display for Case {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} Hz {} x{} viewer {}", self.hz, self.repaint.name(), self.tabs, self.viewer.name())
    }
}

impl Plan {
    fn cases(&self) -> Vec<Case> {
        self.repaints
            .iter()
            .flat_map(|&repaint| self.rates.iter().map(move |&hz| (repaint, hz)))
            .flat_map(|(repaint, hz)| self.tabs.iter().map(move |&tabs| (repaint, hz, tabs)))
            .flat_map(|(repaint, hz, tabs)| self.viewers.iter().map(move |&viewer| Case { hz, repaint, tabs, viewer }))
            .collect()
    }
}

pub(crate) fn run(plan: &Plan, progress: &mut impl Write) -> Result<Vec<Row>> {
    std::fs::create_dir_all(&plan.work).with_context(|| format!("create {}", plan.work.display()))?;
    let work = std::fs::canonicalize(&plan.work).with_context(|| format!("find {}", plan.work.display()))?;
    let mut rows = Vec::new();
    for (run, case) in (1u32..).zip(plan.cases()) {
        let agent = Agent {
            side: "measured binary",
            binary: plan.agent.clone(),
            work: work.clone(),
            home: format!("run-{run}"),
        };
        let home = agent.home();
        let row = measure(plan, agent, case).with_context(|| case.to_string())?;
        writeln!(progress, "{}", line(&row)).context("report the progress")?;
        rows.push(row);
        std::fs::remove_dir_all(&home).with_context(|| format!("remove {}", home.display()))?;
    }
    Ok(rows)
}

pub(crate) fn line(row: &Row) -> String {
    format!(
        "| {} Hz | {} | {} | {} | {:.0} | {:.1} | {:.1} | {:.1} | {:.1} | {} | {:.1} | {:.1} |",
        row.hz,
        row.repaint.name(),
        row.tabs,
        row.viewer.name(),
        row.program_bytes_per_sec / 1024.0,
        row.program_frames_per_sec,
        row.daemon_frames_per_sec,
        row.daemon_cpu_percent,
        row.daemon_rss_mb,
        row.daemon_fds,
        row.stream_messages_per_sec,
        row.stream_bytes_per_sec / 1024.0,
    )
}

pub(crate) const HEADER: &str = "| rate | repaint | tabs | viewer | program KiB/s | program frames/s | daemon frames/s | daemon CPU % | daemon RSS MB | daemon fds | stream msgs/s | stream KiB/s |\n|---|---|---|---|---|---|---|---|---|---|---|---|";

fn measure(plan: &Plan, agent: Agent, case: Case) -> Result<Row> {
    let home = agent.home();
    let mut daemon = Daemon::start(agent)?;
    let tab = open_tabs(plan, &daemon, &home, case)?;
    let readers = match case.viewer {
        Viewer::None => Vec::new(),
        Viewer::Dashboard => {
            let port = daemon.dashboard()?;
            (0..case.tabs).map(|session| Reader::follow(port, &format!("s{session}/{tab}"))).collect::<Result<_>>()?
        }
    };
    run_for(&daemon, case.hz, SETTLE_SECONDS)?;
    let window = Window::watch(&daemon, &home, case, plan.seconds, &readers)?;
    let rss = daemon.memory()?;
    let fds = daemon.fds()?;
    readers.into_iter().try_for_each(Reader::stop)?;
    daemon.stop()?;
    window.row(case, rss, fds)
}

fn open_tabs(plan: &Plan, daemon: &Daemon, home: &Path, case: Case) -> Result<String> {
    let redraw = plan.fixtures.join("redraw");
    let hz = case.hz.to_string();
    let flag = case.repaint.flag();
    let mut opened = None;
    for tab in 0..case.tabs {
        let stats = home.join(format!("s{tab}.stats"));
        let session = format!("s{tab}");
        daemon.agent().new_session(&session)?;
        opened = Some(daemon.agent().open(
            "redraw",
            &[
                "--session",
                &session,
                "--size",
                SIZE,
                "--wait",
                "forward",
                utf8(&redraw)?,
                "--hz",
                &hz,
                &flag,
                "--stats",
                utf8(&stats)?,
            ],
        )?);
    }
    opened.context("a load case opens at least one tab")
}

fn run_for(daemon: &Daemon, hz: u32, seconds: u64) -> Result<()> {
    let until = frame(daemon, "s0")? + u64::from(hz) * seconds;
    let timeout = (seconds * 10 + 10) * 1000;
    daemon.agent().run(&[
        "--session",
        "s0",
        "--timeout",
        &timeout.to_string(),
        "wait",
        "--frame",
        &until.to_string(),
    ])?;
    Ok(())
}

fn frame(daemon: &Daemon, session: &str) -> Result<u64> {
    let frame = daemon.agent().run(&["--session", session, "get", "frame"])?;
    String::from_utf8_lossy(&frame.stdout).trim().parse::<u64>().context("read the frame")
}

struct Window {
    seconds: f64,
    before: Sample,
    after: Sample,
    streamed: (u64, u64),
}

impl Window {
    fn watch(daemon: &Daemon, home: &Path, case: Case, seconds: u64, readers: &[Reader]) -> Result<Self> {
        let before = Sample::take(daemon, home, case.tabs)?;
        let counted: Vec<(u64, u64)> = readers.iter().map(Reader::counts).collect();
        let started = Instant::now();
        run_for(daemon, case.hz, seconds)?;
        let after = Sample::take(daemon, home, case.tabs)?;
        let seconds = started.elapsed().as_secs_f64();
        let mut streamed = (0u64, 0u64);
        for (reader, (messages, bytes)) in readers.iter().zip(&counted) {
            let (now_messages, now_bytes) = reader.counts();
            streamed.0 += now_messages - messages;
            streamed.1 += now_bytes - bytes;
        }
        Ok(Self { seconds, before, after, streamed })
    }

    fn per_sec(&self, count: u64) -> Result<f64> {
        let count = u32::try_from(count).with_context(|| format!("{count} is too many to report as a rate"))?;
        Ok(f64::from(count) / self.seconds)
    }

    fn row(&self, case: Case, rss: f64, fds: usize) -> Result<Row> {
        let (before, after) = (&self.before, &self.after);
        Ok(Row {
            hz: case.hz,
            repaint: case.repaint,
            tabs: case.tabs,
            viewer: case.viewer,
            seconds: self.seconds,
            program_bytes_per_sec: self.per_sec(after.program_bytes - before.program_bytes)?,
            program_frames_per_sec: self.per_sec(after.program_frames - before.program_frames)?,
            daemon_frames_per_sec: self.per_sec(after.frames - before.frames)?,
            daemon_cpu_percent: (after.cpu_seconds - before.cpu_seconds) / self.seconds * 100.0,
            daemon_rss_mb: rss,
            daemon_fds: fds,
            stream_messages_per_sec: self.per_sec(self.streamed.0)?,
            stream_bytes_per_sec: self.per_sec(self.streamed.1)?,
        })
    }
}

struct Sample {
    cpu_seconds: f64,
    frames: u64,
    program_frames: u64,
    program_bytes: u64,
}

impl Sample {
    fn take(daemon: &Daemon, home: &Path, tabs: usize) -> Result<Self> {
        let mut sample = Self { cpu_seconds: daemon.cpu_seconds()?, frames: 0, program_frames: 0, program_bytes: 0 };
        for tab in 0..tabs {
            let session = format!("s{tab}");
            sample.frames += frame(daemon, &session)?;
            let stats = std::fs::read_to_string(home.join(format!("{session}.stats"))).unwrap_or_default();
            let mut counts = stats.split_whitespace().map(|value| value.parse::<u64>().unwrap_or(0));
            sample.program_frames += counts.next().unwrap_or(0);
            sample.program_bytes += counts.next().unwrap_or(0);
        }
        Ok(sample)
    }
}

#[derive(Default)]
struct Counters {
    messages: AtomicU64,
    bytes: AtomicU64,
    stop: AtomicBool,
}

impl Counters {
    fn read(&self, mut websocket: WebSocket<TcpStream>) {
        let mut open = true;
        while open && !self.stop.load(Ordering::Relaxed) {
            open = self.take(&mut websocket);
        }
    }

    fn take(&self, websocket: &mut WebSocket<TcpStream>) -> bool {
        match websocket.read() {
            Ok(Message::Text(text)) => {
                self.messages.fetch_add(1, Ordering::Relaxed);
                self.bytes.fetch_add(text.len() as u64, Ordering::Relaxed);
                !is_frame(&text) || websocket.send(Message::text(SHOWN)).is_ok()
            }
            Ok(_) => true,
            Err(tungstenite::Error::Io(e)) => {
                matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut)
            }
            Err(_) => false,
        }
    }
}

struct Reader {
    counters: Arc<Counters>,
    thread: JoinHandle<()>,
}

impl Reader {
    fn follow(port: u16, tab: &str) -> Result<Self> {
        let url = format!("ws://localhost:{port}/api/session/{tab}/stream");
        let stream = TcpStream::connect(("127.0.0.1", port)).context("connect to the dashboard")?;
        stream.set_read_timeout(Some(Duration::from_millis(200))).context("set a read timeout")?;
        let (websocket, _) = tungstenite::client(url.as_str(), stream).with_context(|| format!("open {url}"))?;
        let counters = Arc::new(Counters::default());
        let counting = Arc::clone(&counters);
        let thread = std::thread::spawn(move || counting.read(websocket));
        Ok(Self { counters, thread })
    }

    fn counts(&self) -> (u64, u64) {
        (self.counters.messages.load(Ordering::Relaxed), self.counters.bytes.load(Ordering::Relaxed))
    }

    fn stop(self) -> Result<()> {
        self.counters.stop.store(true, Ordering::Relaxed);
        self.thread.join().map_err(|panic| {
            let message = panic
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("with no message");
            anyhow!("a stream reader panicked: {message}")
        })
    }
}

const SHOWN: &str = r#"{"type":"shown"}"#;

#[derive(Deserialize)]
struct Update<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
}

fn is_frame(text: &str) -> bool {
    serde_json::from_str::<Update<'_>>(text).is_ok_and(|update| update.kind == "frame")
}
