use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tungstenite::Message;

use crate::daemon::{Agent, Daemon};
use crate::measure::{SIZE, utf8};

const SETTLE: Duration = Duration::from_secs(2);

pub struct Plan {
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
pub enum Repaint {
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

    pub fn parse(text: &str) -> Result<Self> {
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
pub enum Viewer {
    None,
    Dashboard,
}

impl Viewer {
    pub const ALL: [Self; 2] = [Self::None, Self::Dashboard];

    fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Dashboard => "dashboard",
        }
    }

    pub fn parse(text: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|viewer| viewer.name() == text)
            .with_context(|| format!("{text} is not a viewer; use {}", Self::ALL.map(Self::name).join(" or ")))
    }
}

#[derive(Serialize)]
pub struct Row {
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

pub fn run(plan: &Plan) -> Result<Vec<Row>> {
    std::fs::create_dir_all(&plan.work).with_context(|| format!("create {}", plan.work.display()))?;
    let work = std::fs::canonicalize(&plan.work).with_context(|| format!("find {}", plan.work.display()))?;
    let mut rows = Vec::new();
    let mut run = 0;
    for &repaint in &plan.repaints {
        for &hz in &plan.rates {
            for &tabs in &plan.tabs {
                for &viewer in &plan.viewers {
                    run += 1;
                    let agent = Agent {
                        side: "measured binary",
                        binary: plan.agent.clone(),
                        work: work.clone(),
                        home: format!("run-{run}"),
                    };
                    let home = agent.home();
                    let row = measure(plan, agent, hz, repaint, tabs, viewer)
                        .with_context(|| format!("{hz} Hz {} x{tabs} viewer {}", repaint.name(), viewer.name()))?;
                    eprintln!("{}", line(&row));
                    rows.push(row);
                    std::fs::remove_dir_all(&home).with_context(|| format!("remove {}", home.display()))?;
                }
            }
        }
    }
    Ok(rows)
}

pub fn line(row: &Row) -> String {
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

pub const HEADER: &str = "| rate | repaint | tabs | viewer | program KiB/s | program frames/s | daemon frames/s | daemon CPU % | daemon RSS MB | daemon fds | stream msgs/s | stream KiB/s |\n|---|---|---|---|---|---|---|---|---|---|---|---|";

fn measure(plan: &Plan, agent: Agent, hz: u32, repaint: Repaint, tabs: usize, viewer: Viewer) -> Result<Row> {
    let home = agent.home();
    let daemon = Daemon::start(agent)?;
    let redraw = plan.fixtures.join("redraw");
    let hz_text = hz.to_string();
    let flag = repaint.flag();
    for tab in 0..tabs {
        let stats = home.join(format!("s{tab}.stats"));
        daemon.agent.run(&[
            "--session",
            &format!("s{tab}"),
            "open",
            "--size",
            SIZE,
            "--wait",
            "ready",
            utf8(&redraw)?,
            "--hz",
            &hz_text,
            &flag,
            "--stats",
            utf8(&stats)?,
        ])?;
    }
    let readers = match viewer {
        Viewer::None => Vec::new(),
        Viewer::Dashboard => {
            let port = daemon.dashboard()?;
            (0..tabs).map(|tab| Reader::follow(port, &format!("s{tab}"))).collect::<Result<_>>()?
        }
    };
    std::thread::sleep(SETTLE);

    let before = Sample::take(&daemon, &home, tabs)?;
    let counted: Vec<(u64, u64)> = readers.iter().map(Reader::counts).collect();
    let started = Instant::now();
    std::thread::sleep(Duration::from_secs(plan.seconds));
    let after = Sample::take(&daemon, &home, tabs)?;
    let seconds = started.elapsed().as_secs_f64();
    let mut streamed = (0u64, 0u64);
    for (reader, (messages, bytes)) in readers.iter().zip(&counted) {
        let (now_messages, now_bytes) = reader.counts();
        streamed.0 += now_messages - messages;
        streamed.1 += now_bytes - bytes;
    }
    let rss = daemon.memory()?;
    let fds = daemon.fds()?;
    for reader in readers {
        reader.stop();
    }
    drop(daemon);

    let per_sec = |value: f64| value / seconds;
    Ok(Row {
        hz,
        repaint,
        tabs,
        viewer,
        seconds,
        program_bytes_per_sec: per_sec((after.program_bytes - before.program_bytes) as f64),
        program_frames_per_sec: per_sec((after.program_frames - before.program_frames) as f64),
        daemon_frames_per_sec: per_sec((after.frames - before.frames) as f64),
        daemon_cpu_percent: per_sec(after.cpu_seconds - before.cpu_seconds) * 100.0,
        daemon_rss_mb: rss,
        daemon_fds: fds,
        stream_messages_per_sec: per_sec(streamed.0 as f64),
        stream_bytes_per_sec: per_sec(streamed.1 as f64),
    })
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
            let frame = daemon.agent.run(&["--session", &session, "get", "frame"])?;
            sample.frames += String::from_utf8_lossy(&frame.stdout).trim().parse::<u64>().context("read the frame")?;
            let stats = std::fs::read_to_string(home.join(format!("{session}.stats"))).unwrap_or_default();
            let mut counts = stats.split_whitespace().map(|value| value.parse::<u64>().unwrap_or(0));
            sample.program_frames += counts.next().unwrap_or(0);
            sample.program_bytes += counts.next().unwrap_or(0);
        }
        Ok(sample)
    }
}

struct Reader {
    messages: Arc<AtomicU64>,
    bytes: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl Reader {
    fn follow(port: u16, session: &str) -> Result<Self> {
        let url = format!("ws://localhost:{port}/api/session/{session}/t1/stream");
        let stream = TcpStream::connect(("127.0.0.1", port)).context("connect to the dashboard")?;
        stream.set_read_timeout(Some(Duration::from_millis(200))).context("set a read timeout")?;
        let (mut websocket, _) = tungstenite::client(url.as_str(), stream).with_context(|| format!("open {url}"))?;
        let messages = Arc::new(AtomicU64::new(0));
        let bytes = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (counted, sized, stopping) = (Arc::clone(&messages), Arc::clone(&bytes), Arc::clone(&stop));
        let thread = std::thread::spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                match websocket.read() {
                    Ok(Message::Text(text)) => {
                        counted.fetch_add(1, Ordering::Relaxed);
                        sized.fetch_add(text.len() as u64, Ordering::Relaxed);
                        if let Some(shown) = acknowledgement(&text)
                            && websocket.send(Message::text(shown)).is_err()
                        {
                            return;
                        }
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(e))
                        if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                    Err(_) => return,
                }
            }
        });
        Ok(Self { messages, bytes, stop, thread })
    }

    fn counts(&self) -> (u64, u64) {
        (self.messages.load(Ordering::Relaxed), self.bytes.load(Ordering::Relaxed))
    }

    fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        if self.thread.join().is_err() {
            eprintln!("a stream reader panicked");
        }
    }
}

#[derive(Deserialize)]
struct Update<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    frame: Option<u64>,
}

fn acknowledgement(text: &str) -> Option<String> {
    let update: Update = serde_json::from_str(text).ok()?;
    let frame = update.frame.filter(|_| update.kind == "frame")?;
    Some(format!(r#"{{"type":"shown","frame":{frame}}}"#))
}
