use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::json;

use crate::protocol::Size;

const RESET: &str = "\x1bc";

pub struct Recording {
    writer: Mutex<Writer>,
}

struct Writer {
    tape: Tape,
    undecoded: Vec<u8>,
}

struct Tape {
    path: PathBuf,
    file: Result<File, String>,
    clock: Clock,
    length: u64,
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
        let header = format!("{}\n", serde_json::to_string(&header)?);
        file.write_all(header.as_bytes()).with_context(|| format!("write to {}", path.display()))?;
        Ok(Self { path: path.to_path_buf(), file: Ok(file), clock, length: header.len() as u64 })
    }

    fn event(&mut self, kind: &str, data: &str) {
        let Ok(file) = &mut self.file else {
            return;
        };
        let line = format!("{}\n", json!([self.clock.seconds(), kind, data]));
        match file.write_all(line.as_bytes()) {
            Ok(()) => self.length += line.len() as u64,
            Err(e) => {
                eprintln!("writing the recording {} failed: {e}; nothing more is recorded there", self.path.display());
                self.file = Err(e.to_string());
            }
        }
    }
}

impl Recording {
    pub fn start(path: PathBuf, size: Size, command: String) -> Arc<Self> {
        let tape = Tape::create(&path, size, &command, Clock::on_the_second()).unwrap_or_else(|e| {
            eprintln!("starting the recording {} failed: {e:#}", path.display());
            Tape { path, file: Err(format!("{e:#}")), clock: Clock::on_the_second(), length: 0 }
        });
        let writer = Writer { tape, undecoded: Vec::new() };
        Arc::new(Self { writer: Mutex::new(writer) })
    }

    pub fn output(&self, bytes: &[u8]) -> u64 {
        let mut writer = self.writer.lock().unwrap();
        let text = decode(&mut writer.undecoded, bytes);
        if !text.is_empty() {
            writer.tape.event("o", &text);
        }
        writer.tape.length
    }

    pub fn input(&self, bytes: &[u8]) {
        self.writer.lock().unwrap().tape.event("i", &String::from_utf8_lossy(bytes));
    }

    pub fn mark(&self, name: &str) {
        self.writer.lock().unwrap().tape.event("m", name);
    }

    pub fn resize(&self, size: Size) -> u64 {
        let mut writer = self.writer.lock().unwrap();
        writer.tape.event("r", &format!("{}x{}", size.cols, size.rows));
        writer.tape.length
    }

    pub fn length(&self) -> u64 {
        self.writer.lock().unwrap().tape.length
    }

    pub fn cast(&self) -> PathBuf {
        self.writer.lock().unwrap().tape.path.clone()
    }

    pub fn restart(&self) {
        let mut writer = self.writer.lock().unwrap();
        let mut text = String::from_utf8_lossy(&std::mem::take(&mut writer.undecoded)).into_owned();
        text.push_str(RESET);
        writer.tape.event("o", &text);
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
