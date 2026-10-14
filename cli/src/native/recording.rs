use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use serde_json::json;

use crate::protocol::Size;

const RESET: &str = "\x1bc";

pub struct Recording {
    tab: String,
    writer: Mutex<Writer>,
}

pub struct Saved {
    pub path: PathBuf,
    pub duration: Duration,
    pub bytes: u64,
}

struct Writer {
    tape: Tape,
    first_size: Size,
    command: String,
    undecoded: Vec<u8>,
    copy: Option<Copy>,
}

struct Copy {
    tape: Tape,
    started: Instant,
}

struct Tape {
    path: PathBuf,
    file: Result<File, String>,
    clock: Clock,
}

#[derive(Serialize)]
struct Header<'a> {
    version: u8,
    width: u16,
    height: u16,
    timestamp: u64,
    command: &'a str,
}

struct Clock {
    timestamp: u64,
    origin: Instant,
    offset: f64,
}

impl Clock {
    fn on_the_second() -> Self {
        let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_else(|e| {
            eprintln!("the system clock is {:?} before 1970; recordings count their time from 1970", e.duration());
            Duration::ZERO
        });
        Self {
            timestamp: since_epoch.as_secs(),
            origin: Instant::now(),
            offset: f64::from(since_epoch.subsec_micros()) / 1e6,
        }
    }

    fn from_now() -> Self {
        Self { offset: 0.0, ..Self::on_the_second() }
    }

    fn seconds(&self) -> f64 {
        ((self.offset + self.origin.elapsed().as_secs_f64()) * 1e6).round() / 1e6
    }
}

impl Tape {
    fn create(path: &Path, size: Size, command: &str, clock: Clock) -> Result<Self> {
        let header = Header { version: 2, width: size.cols, height: size.rows, timestamp: clock.timestamp, command };
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).with_context(|| format!("create {}", folder.display()))?;
        }
        let mut file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        let header = serde_json::to_string(&header)?;
        writeln!(file, "{header}").with_context(|| format!("write to {}", path.display()))?;
        Ok(Self { path: path.to_path_buf(), file: Ok(file), clock })
    }

    fn event(&mut self, kind: &str, data: &str) {
        let Ok(file) = &mut self.file else {
            return;
        };
        let line = format!("{}\n", json!([self.clock.seconds(), kind, data]));
        if let Err(e) = file.write_all(line.as_bytes()) {
            eprintln!("writing the recording {} failed: {e}; nothing more is recorded there", self.path.display());
            self.file = Err(e.to_string());
        }
    }

    fn failure(&self) -> Option<anyhow::Error> {
        let reason = self.file.as_ref().err()?;
        Some(anyhow!("writing {} failed: {reason}", self.path.display()))
    }
}

impl Recording {
    pub fn start(tab: String, path: PathBuf, size: Size, command: String) -> Arc<Self> {
        let tape = Tape::create(&path, size, &command, Clock::on_the_second()).unwrap_or_else(|e| {
            eprintln!("starting the recording {} failed: {e:#}", path.display());
            Tape { path, file: Err(format!("{e:#}")), clock: Clock::on_the_second() }
        });
        let writer = Writer { tape, first_size: size, command, undecoded: Vec::new(), copy: None };
        Arc::new(Self { tab, writer: Mutex::new(writer) })
    }

    pub fn output(&self, bytes: &[u8]) {
        let mut writer = self.writer.lock().unwrap();
        let text = decode(&mut writer.undecoded, bytes);
        if !text.is_empty() {
            writer.event("o", &text);
        }
    }

    pub fn input(&self, bytes: &[u8]) {
        self.writer.lock().unwrap().event("i", &String::from_utf8_lossy(bytes));
    }

    pub fn resize(&self, size: Size) {
        self.writer.lock().unwrap().event("r", &format!("{}x{}", size.cols, size.rows));
    }

    pub fn restart(&self) {
        let mut writer = self.writer.lock().unwrap();
        let mut text = String::from_utf8_lossy(&std::mem::take(&mut writer.undecoded)).into_owned();
        text.push_str(RESET);
        writer.event("o", &text);
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        let mut writer = self.writer.lock().unwrap();
        if let Some(copy) = &writer.copy {
            bail!(
                "{} is already being recorded to {}; stop with `agent-terminal record stop`",
                self.tab,
                copy.tape.path.display()
            );
        }
        if let Some(failure) = writer.tape.failure() {
            return Err(failure.context(format!("{} has no recording to save", self.tab)));
        }
        let recorded = std::fs::read_to_string(&writer.tape.path)
            .with_context(|| format!("read the recording {}", writer.tape.path.display()))?;
        let mut tape = Tape::create(path, writer.first_size, &writer.command, Clock::from_now())?;
        for line in recorded.lines().skip(1) {
            let (_, kind, data): (f64, String, String) = serde_json::from_str(line)
                .with_context(|| format!("read the recording {}", writer.tape.path.display()))?;
            if kind != "i" {
                tape.event(&kind, &data);
            }
        }
        if let Some(failure) = tape.failure() {
            return Err(failure);
        }
        writer.copy = Some(Copy { tape, started: Instant::now() });
        Ok(())
    }

    pub fn saving_to(&self) -> Option<PathBuf> {
        self.writer.lock().unwrap().copy.as_ref().map(|copy| copy.tape.path.clone())
    }

    pub fn stop_saving(&self) -> Result<Saved> {
        let copy = self.writer.lock().unwrap().copy.take().ok_or_else(|| {
            anyhow!("{} is not being recorded to a file; start with `agent-terminal record start [path]`", self.tab)
        })?;
        if let Some(failure) = copy.tape.failure() {
            return Err(failure);
        }
        let path = copy.tape.path;
        let bytes = std::fs::metadata(&path).with_context(|| format!("read {}", path.display()))?.len();
        Ok(Saved { path, duration: copy.started.elapsed(), bytes })
    }
}

impl Writer {
    fn event(&mut self, kind: &str, data: &str) {
        self.tape.event(kind, data);
        if let Some(copy) = &mut self.copy {
            copy.tape.event(kind, data);
        }
    }
}

pub fn decode(undecoded: &mut Vec<u8>, bytes: &[u8]) -> String {
    let mut pending = std::mem::take(undecoded);
    pending.extend_from_slice(bytes);
    let mut text = String::new();
    let mut start = 0;
    loop {
        match std::str::from_utf8(&pending[start..]) {
            Ok(valid) => {
                text.push_str(valid);
                return text;
            }
            Err(e) => {
                let valid_end = start + e.valid_up_to();
                text.push_str(&String::from_utf8_lossy(&pending[start..valid_end]));
                match e.error_len() {
                    Some(len) => {
                        text.push(char::REPLACEMENT_CHARACTER);
                        start = valid_end + len;
                    }
                    None => {
                        *undecoded = pending.split_off(valid_end);
                        return text;
                    }
                }
            }
        }
    }
}
