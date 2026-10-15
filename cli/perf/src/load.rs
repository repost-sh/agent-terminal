use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use serde::Serialize;
use tungstenite::Message;

const SIZE: &str = "120x40";
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
    pub fn parse(text: &str) -> Result<Self> {
        match text {
            "partial" => Ok(Self::Partial),
            "full" => Ok(Self::Full),
            other => anyhow::bail!("{other} is not a repaint; use partial or full"),
        }
    }

    fn flag(self) -> &'static str {
        match self {
            Self::Partial => "--partial",
            Self::Full => "--full",
        }
    }
}

#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Viewer {
    None,
    Live,
    Dashboard,
}

impl Viewer {
    pub fn parse(text: &str) -> Result<Self> {
        match text {
            "none" => Ok(Self::None),
            "live" => Ok(Self::Live),
            "dashboard" => Ok(Self::Dashboard),
            other => anyhow::bail!("{other} is not a viewer; use none, live or dashboard"),
        }
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
    pub cast_bytes_per_sec: f64,
    pub cast_mb_per_hour: f64,
    pub stream_messages_per_sec: f64,
    pub stream_bytes_per_sec: f64,
}

pub fn run(plan: &Plan) -> Result<Vec<Row>> {
    let mut rows = Vec::new();
    let mut run = 0;
    for &repaint in &plan.repaints {
        for &hz in &plan.rates {
            for &tabs in &plan.tabs {
                for &viewer in &plan.viewers {
                    run += 1;
                    let home = plan.work.join(format!("run-{run}"));
                    let row = measure(plan, &home, hz, repaint, tabs, viewer)
                        .with_context(|| format!("{hz} Hz {} x{tabs} viewer {}", repaint.flag(), name(viewer)))?;
                    eprintln!("{}", line(&row));
                    rows.push(row);
                    std::fs::remove_dir_all(&home).with_context(|| format!("remove {}", home.display()))?;
                }
            }
        }
    }
    Ok(rows)
}

fn name(viewer: Viewer) -> &'static str {
    match viewer {
        Viewer::None => "none",
        Viewer::Live => "live",
        Viewer::Dashboard => "dashboard",
    }
}

pub fn line(row: &Row) -> String {
    format!(
        "| {} Hz | {} | {} | {} | {:.0} | {:.1} | {:.1} | {:.1} | {:.1} | {} | {:.1} | {:.1} | {:.1} |",
        row.hz,
        match row.repaint {
            Repaint::Partial => "partial",
            Repaint::Full => "full",
        },
        row.tabs,
        name(row.viewer),
        row.program_bytes_per_sec / 1024.0,
        row.program_frames_per_sec,
        row.daemon_frames_per_sec,
        row.daemon_cpu_percent,
        row.daemon_rss_mb,
        row.daemon_fds,
        row.cast_mb_per_hour,
        row.stream_messages_per_sec,
        row.stream_bytes_per_sec / 1024.0,
    )
}

pub const HEADER: &str = "| rate | repaint | tabs | viewer | program KiB/s | program frames/s | daemon frames/s | daemon CPU % | daemon RSS MB | daemon fds | .cast MB/hour | stream msgs/s | stream KiB/s |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|";

fn measure(plan: &Plan, home: &Path, hz: u32, repaint: Repaint, tabs: usize, viewer: Viewer) -> Result<Row> {
    let daemon = Daemon::start(&plan.agent, home)?;
    let redraw = plan.fixtures.join("redraw");
    let hz_text = hz.to_string();
    for tab in 0..tabs {
        let stats = home.join(format!("s{tab}.stats"));
        daemon.run(&[
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
            repaint.flag(),
            "--stats",
            utf8(&stats)?,
        ])?;
    }
    let readers = match viewer {
        Viewer::None => Vec::new(),
        Viewer::Live | Viewer::Dashboard => {
            let port = daemon.dashboard()?;
            (0..tabs).map(|tab| Reader::follow(port, &format!("s{tab}"))).collect::<Result<_>>()?
        }
    };
    std::thread::sleep(SETTLE);

    let before = Sample::take(&daemon, home, tabs)?;
    let counted: Vec<(u64, u64)> = readers.iter().map(Reader::counts).collect();
    let started = Instant::now();
    std::thread::sleep(Duration::from_secs(plan.seconds));
    let after = Sample::take(&daemon, home, tabs)?;
    let seconds = started.elapsed().as_secs_f64();
    let mut streamed = (0u64, 0u64);
    for (reader, (messages, bytes)) in readers.iter().zip(&counted) {
        let (now_messages, now_bytes) = reader.counts();
        streamed.0 += now_messages - messages;
        streamed.1 += now_bytes - bytes;
    }
    let rss = daemon.rss_mb()?;
    let fds = daemon.fds()?;
    for reader in readers {
        reader.stop();
    }
    drop(daemon);

    let per_sec = |value: f64| value / seconds;
    let cast_bytes_per_sec = per_sec((after.cast_bytes - before.cast_bytes) as f64);
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
        cast_bytes_per_sec,
        cast_mb_per_hour: cast_bytes_per_sec * 3600.0 / 1_000_000.0,
        stream_messages_per_sec: per_sec(streamed.0 as f64),
        stream_bytes_per_sec: per_sec(streamed.1 as f64),
    })
}

struct Sample {
    cpu_seconds: f64,
    frames: u64,
    program_frames: u64,
    program_bytes: u64,
    cast_bytes: u64,
}

impl Sample {
    fn take(daemon: &Daemon, home: &Path, tabs: usize) -> Result<Self> {
        let mut sample =
            Self { cpu_seconds: daemon.cpu_seconds()?, frames: 0, program_frames: 0, program_bytes: 0, cast_bytes: 0 };
        for tab in 0..tabs {
            let session = format!("s{tab}");
            let frame = daemon.run(&["--session", &session, "get", "frame"])?;
            sample.frames += String::from_utf8_lossy(&frame.stdout).trim().parse::<u64>().context("read the frame")?;
            let stats = std::fs::read_to_string(home.join(format!("{session}.stats"))).unwrap_or_default();
            let mut counts = stats.split_whitespace().map(|value| value.parse::<u64>().unwrap_or(0));
            sample.program_frames += counts.next().unwrap_or(0);
            sample.program_bytes += counts.next().unwrap_or(0);
            let cast = home.join("sessions").join(&session).join("t1.cast");
            sample.cast_bytes += std::fs::metadata(&cast).with_context(|| format!("read {}", cast.display()))?.len();
        }
        Ok(sample)
    }
}

struct Daemon {
    agent: PathBuf,
    home: PathBuf,
    process: Child,
    _ready: BufReader<ChildStdout>,
}

impl Daemon {
    fn start(agent: &Path, home: &Path) -> Result<Self> {
        std::fs::create_dir_all(home).with_context(|| format!("create {}", home.display()))?;
        let log = std::fs::File::create(home.join("daemon.log")).context("create the daemon log")?;
        let mut process = command(agent, home)
            .args(["daemon", "run"])
            .stdout(Stdio::piped())
            .stderr(log)
            .spawn()
            .context("start the daemon")?;
        let mut ready = BufReader::new(process.stdout.take().context("daemon stdout")?);
        let mut line = String::new();
        ready.read_line(&mut line).context("wait for the daemon")?;
        ensure!(line.trim() == "ready", "the daemon did not start: {line:?}");
        Ok(Self { agent: agent.to_path_buf(), home: home.to_path_buf(), process, _ready: ready })
    }

    fn run(&self, args: &[&str]) -> Result<std::process::Output> {
        let output = command(&self.agent, &self.home).args(args).output().context("run agent-terminal")?;
        ensure!(
            output.status.success(),
            "`agent-terminal {}` failed: {}{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        Ok(output)
    }

    fn dashboard(&self) -> Result<u16> {
        let output = self.run(&["--json", "dashboard", "start", "--port", "0"])?;
        let started: serde_json::Value = serde_json::from_slice(&output.stdout).context("read the dashboard")?;
        let port = started["port"].as_u64().context("the dashboard's port")?;
        Ok(u16::try_from(port)?)
    }

    fn ps(&self, field: &str) -> Result<String> {
        let output =
            Command::new("ps").args(["-o", field, "-p", &self.process.id().to_string()]).output().context("run ps")?;
        ensure!(output.status.success(), "ps found no daemon");
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn cpu_seconds(&self) -> Result<f64> {
        let time = self.ps("time=")?;
        let mut seconds = 0.0;
        for part in time.split(':') {
            seconds = seconds * 60.0 + part.parse::<f64>().with_context(|| format!("read the CPU time {time:?}"))?;
        }
        Ok(seconds)
    }

    fn rss_mb(&self) -> Result<f64> {
        let kibibytes: f64 = self.ps("rss=")?.parse().context("read the resident size")?;
        Ok(kibibytes * 1024.0 / 1_000_000.0)
    }

    fn fds(&self) -> Result<usize> {
        let output = Command::new("lsof")
            .args(["-n", "-P", "-p", &self.process.id().to_string()])
            .output()
            .context("run lsof")?;
        Ok(String::from_utf8_lossy(&output.stdout).lines().count().saturating_sub(1))
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if let Err(e) = self.run(&["daemon", "stop"]) {
            eprintln!("stopping the daemon failed: {e:#}");
            if let Err(e) = self.process.kill() {
                eprintln!("killing the daemon failed: {e}");
            }
        }
        if let Err(e) = self.process.wait() {
            eprintln!("waiting for the daemon failed: {e}");
        }
    }
}

fn command(agent: &Path, home: &Path) -> Command {
    let mut command = Command::new(agent);
    command
        .env("AGENT_TERMINAL_HOME", home)
        .env_remove("AGENT_TERMINAL_SESSION")
        .env_remove("AGENT_TERMINAL_SCREENSHOT_DIR")
        .stdin(Stdio::null());
    command
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

fn acknowledgement(text: &str) -> Option<String> {
    let message: serde_json::Value = serde_json::from_str(text).ok()?;
    (message["type"] == "frame").then(|| serde_json::json!({ "type": "shown", "frame": message["frame"] }).to_string())
}

fn utf8(path: &Path) -> Result<&str> {
    path.to_str().with_context(|| format!("{} is not UTF-8", path.display()))
}
