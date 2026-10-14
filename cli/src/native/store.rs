use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use tokio::sync::watch;
use zip::write::SimpleFileOptions;

use crate::native::changes::Changes;
use crate::native::pty;
use crate::native::recording::Recording;
use crate::output::call_line;

pub struct SessionFolder {
    session: String,
    path: PathBuf,
    last_tab: AtomicU32,
    activity: Mutex<()>,
    logged: Changes,
}

#[derive(Serialize)]
pub struct ActivityEntry {
    pub time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab: Option<String>,
    pub command: String,
    pub duration_ms: u64,
    pub exit: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<u64>,
    pub output: String,
}

const KEPT_OUTPUT_BYTES: usize = 64 * 1024;

impl ActivityEntry {
    pub fn at(time: SystemTime, typed: &[String], duration: Duration) -> Self {
        Self {
            time: humantime::format_rfc3339_millis(time).to_string(),
            tab: None,
            command: call_line(typed),
            duration_ms: duration.as_millis() as u64,
            exit: 0,
            error: None,
            frame: None,
            output: String::new(),
        }
    }

    pub fn printed(&mut self, mut text: String) {
        if text.len() > KEPT_OUTPUT_BYTES {
            let kept = text.floor_char_boundary(KEPT_OUTPUT_BYTES);
            let left = text.len() - kept;
            text.truncate(kept);
            text.push_str(&format!("\n… {left} more bytes not kept\n"));
        }
        self.output = text;
    }
}

impl SessionFolder {
    pub fn start(home: &Path, session: &str) -> Result<Self> {
        let path = folder(home, session);
        match std::fs::remove_dir_all(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("clear the previous session folder {}", path.display())),
        }
        std::fs::create_dir_all(&path).with_context(|| format!("create the session folder {}", path.display()))?;
        Ok(Self {
            session: session.to_string(),
            path,
            last_tab: AtomicU32::new(0),
            activity: Mutex::new(()),
            logged: Changes::default(),
        })
    }

    pub fn next_tab(&self) -> u32 {
        self.last_tab.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn log(&self, entry: &ActivityEntry) {
        let path = self.path.join("activity.jsonl");
        let appended = serde_json::to_string(entry).map_err(anyhow::Error::from).and_then(|line| {
            let _turn = self.activity.lock().unwrap();
            let mut log = OpenOptions::new().create(true).append(true).open(&path)?;
            log.write_all(format!("{line}\n").as_bytes())?;
            Ok(())
        });
        match appended {
            Ok(()) => self.logged.notify(),
            Err(e) => eprintln!("adding `{}` to the activity log {} failed: {e:#}", entry.command, path.display()),
        }
    }

    pub fn logged(&self) -> watch::Receiver<u64> {
        self.logged.subscribe()
    }

    pub fn activity_since(&self, offset: u64) -> Result<(Vec<serde_json::Value>, u64)> {
        let path = self.path.join("activity.jsonl");
        let mut log = match File::open(&path) {
            Ok(log) => log,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok((Vec::new(), offset)),
            Err(e) => return Err(e).with_context(|| format!("open {}", path.display())),
        };
        let mut read = Vec::new();
        log.seek(SeekFrom::Start(offset))
            .and_then(|_| log.read_to_end(&mut read))
            .with_context(|| format!("read {}", path.display()))?;
        let complete = read.iter().rposition(|byte| *byte == b'\n').map_or(0, |end| end + 1);
        let entries = read[..complete]
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(serde_json::from_slice)
            .collect::<Result<Vec<_>, _>>()
            .with_context(|| format!("read {}", path.display()))?;
        Ok((entries, offset + complete as u64))
    }

    pub fn record(&self, tab: &str, command: &pty::Command) -> Arc<Recording> {
        let mut typed = vec![command.program.clone()];
        typed.extend(command.args.iter().cloned());
        let path = self.path.join(format!("{tab}.cast"));
        Recording::start(format!("{} {tab}", self.session), path, command.size, call_line(&typed))
    }
}

pub struct Exported {
    pub files: usize,
    pub bytes: u64,
}

pub fn export(home: &Path, session: &str, to: &Path) -> Result<Exported> {
    let folder = folder(home, session);
    if !folder.is_dir() {
        bail!(
            "session {session} has nothing to export; open a terminal in it first with `agent-terminal open <program>`"
        );
    }
    let mut files = Vec::new();
    files_in(&folder, &mut files)?;
    let root = file_name(session);
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for file in &files {
        let mut contents = std::fs::read(file).with_context(|| format!("read {}", file.display()))?;
        if is_appended_to(file) {
            contents.truncate(contents.iter().rposition(|byte| *byte == b'\n').map_or(0, |end| end + 1));
        }
        let name = file
            .strip_prefix(&folder)
            .with_context(|| format!("{} is outside {}", file.display(), folder.display()))?;
        zip.start_file(format!("{root}/{}", name.display()), options)?;
        zip.write_all(&contents)?;
    }
    let zipped = zip.finish()?.into_inner();
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(to, &zipped).with_context(|| format!("write {}", to.display()))?;
    Ok(Exported { files: files.len(), bytes: zipped.len() as u64 })
}

fn is_appended_to(file: &Path) -> bool {
    file.extension().is_some_and(|extension| extension == "cast" || extension == "jsonl")
}

fn files_in(folder: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = std::fs::read_dir(folder)
        .and_then(|entries| entries.map(|entry| entry.map(|entry| entry.path())).collect::<std::io::Result<Vec<_>>>())
        .with_context(|| format!("list {}", folder.display()))?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            files_in(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

pub fn folder(home: &Path, session: &str) -> PathBuf {
    home.join("sessions").join(file_name(session))
}

pub fn file_name(session: &str) -> String {
    if session.is_empty() {
        return "%".into();
    }
    let mut name = String::with_capacity(session.len());
    for (i, byte) in session.bytes().enumerate() {
        let kept = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') || (byte == b'.' && i > 0);
        if kept {
            name.push(byte as char);
        } else {
            name.push_str(&format!("%{byte:02X}"));
        }
    }
    name
}
