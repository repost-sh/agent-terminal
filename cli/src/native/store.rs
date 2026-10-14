use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use zip::write::SimpleFileOptions;

use crate::native::pty;
use crate::native::recording::Recording;
use crate::output::call_line;
use crate::protocol::CommandRecord;

pub struct SessionFolder {
    session: String,
    path: PathBuf,
    last_tab: AtomicU32,
    activity: Mutex<()>,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub screenshots: Vec<PathBuf>,
}

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
            screenshots: Vec::new(),
        }
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
        Ok(Self { session: session.to_string(), path, last_tab: AtomicU32::new(0), activity: Mutex::new(()) })
    }

    pub fn next_tab(&self) -> u32 {
        self.last_tab.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn log(&self, entry: &ActivityEntry) {
        let path = self.path.join("activity.jsonl");
        let appended = serde_json::to_string(entry).map_err(anyhow::Error::from).and_then(|line| {
            let _turn = self.activity.lock().unwrap();
            append_line(&path, &line)
        });
        if let Err(e) = appended {
            eprintln!("adding `{}` to the activity log {} failed: {e:#}", entry.command, path.display());
        }
    }

    pub fn record(&self, tab: &str, command: &pty::Command) -> Arc<Recording> {
        let mut typed = vec![command.program.clone()];
        typed.extend(command.args.iter().cloned());
        let path = self.path.join(format!("{tab}.cast"));
        Recording::start(format!("{} {tab}", self.session), path, command.size, call_line(&typed))
    }

    pub fn command_log(&self, tab: &str) -> CommandLog {
        CommandLog { path: self.path.join(format!("{tab}.commands.jsonl")) }
    }
}

#[derive(Clone)]
pub struct CommandLog {
    path: PathBuf,
}

#[derive(Serialize)]
struct LoggedCommand<'a> {
    #[serde(flatten)]
    record: &'a CommandRecord,
    output: &'a str,
}

impl CommandLog {
    pub fn append(&self, record: &CommandRecord, output: &str) {
        let appended = serde_json::to_string(&LoggedCommand { record, output })
            .map_err(anyhow::Error::from)
            .and_then(|line| append_line(&self.path, &line));
        if let Err(e) = appended {
            eprintln!(
                "adding command {} `{}` to the command log {} failed: {e:#}",
                record.id,
                record.command,
                self.path.display()
            );
        }
    }
}

fn append_line(path: &Path, line: &str) -> Result<()> {
    let mut log = OpenOptions::new().create(true).append(true).open(path)?;
    log.write_all(format!("{line}\n").as_bytes())?;
    Ok(())
}

pub fn write_whole(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let name = path.file_name().with_context(|| format!("{} names no file", path.display()))?;
    static WRITES: AtomicU64 = AtomicU64::new(0);
    let write = WRITES.fetch_add(1, Ordering::Relaxed);
    let partial = parent.join(format!(".{}.{}-{write}.partial", name.to_string_lossy(), std::process::id()));
    std::fs::write(&partial, bytes).with_context(|| format!("write {}", partial.display()))?;
    std::fs::rename(&partial, path).with_context(|| format!("move {} to {}", partial.display(), path.display()))
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
