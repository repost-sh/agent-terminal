mod tab_id;

use std::fmt::{self, Write as _};
use std::fs::{File, OpenOptions};
use std::io::{self, Cursor, ErrorKind, Write};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::native::changes::Changes;
use crate::native::log;
use crate::native::render::Cells;
use crate::output::call_line;
use crate::protocol::{CommandRecord, HumanInput, Name, request};

pub(crate) use tab_id::TabId;

pub(crate) struct SessionFolder {
    path: PathBuf,
    last_tab: Mutex<Option<TabId>>,
    activity: Mutex<Calls>,
    logged: Changes,
    screenshots: Mutex<()>,
    screenshots_listed: Changes,
}

#[derive(Default)]
struct Calls {
    next: u64,
    pending: Vec<(u64, PendingCall)>,
}

#[derive(Serialize, Clone, PartialEq, Eq)]
pub(crate) struct PendingCall {
    pub time: String,
    #[serde(skip)]
    pub tab_id: Option<TabId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab: Option<Name>,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

pub(crate) struct Pending {
    folder: Arc<SessionFolder>,
    id: u64,
}

impl Pending {
    pub(crate) fn log(self, entry: &ActivityEntry) {
        self.folder.append(entry, Some(self.id));
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        let mut calls = self.folder.activity.lock().unwrap_or_else(PoisonError::into_inner);
        let before = calls.pending.len();
        calls.pending.retain(|(id, _)| *id != self.id);
        let ended = calls.pending.len() != before;
        drop(calls);
        if ended {
            self.folder.logged.notify();
        }
    }
}

#[derive(Serialize)]
pub(crate) struct ActivityEntry {
    pub time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<TabId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab: Option<Name>,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    pub duration_ms: u64,
    pub exit: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub screenshots: Vec<PathBuf>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub raw: bool,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub human: Option<HumanInput>,
}

impl ActivityEntry {
    pub(crate) fn logged_for(entry: &serde_json::Value, tab: TabId) -> bool {
        entry.get("tab_id").and_then(|id| TabId::deserialize(id).ok()) == Some(tab)
    }
}

const KEPT_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScreenshotEntry {
    pub time: String,
    pub tab: String,
    pub frame: u64,
    pub path: PathBuf,
    pub by: Option<String>,
    pub width: u32,
    pub height: u32,
    #[serde(flatten)]
    pub cells: Cells,
}

pub(crate) struct Typed<'a> {
    pub time: SystemTime,
    pub words: &'a [String],
    pub action: Option<&'a str>,
}

impl ActivityEntry {
    pub(crate) fn at(typed: &Typed<'_>, duration: Duration) -> Self {
        Self {
            time: humantime::format_rfc3339_millis(typed.time).to_string(),
            tab_id: None,
            tab: None,
            command: call_line(typed.words),
            action: typed.action.map(str::to_string),
            duration_ms: u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
            exit: 0,
            error: None,
            frame: None,
            screenshots: Vec::new(),
            raw: false,
            output: String::new(),
            human: None,
        }
    }

    pub(crate) fn printed(&mut self, mut text: String) {
        if text.len() > KEPT_OUTPUT_BYTES {
            let kept = text.floor_char_boundary(KEPT_OUTPUT_BYTES);
            let left = text.len() - kept;
            text.truncate(kept);
            text = format!("{text}\n… {left} more bytes not kept\n");
        }
        self.output = text;
    }
}

impl SessionFolder {
    pub(crate) fn start(home: &Path, session: &str) -> Result<Self> {
        let path = folder(home, session);
        match std::fs::remove_dir_all(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("clear the previous session folder {}", path.display())),
        }
        std::fs::create_dir_all(&path).with_context(|| format!("create the session folder {}", path.display()))?;
        Ok(Self {
            path,
            last_tab: Mutex::new(None),
            activity: Mutex::new(Calls::default()),
            logged: Changes::default(),
            screenshots: Mutex::new(()),
            screenshots_listed: Changes::default(),
        })
    }

    pub(crate) fn next_tab(&self) -> TabId {
        let mut last = self.last_tab.lock().unwrap_or_else(PoisonError::into_inner);
        let next = last.map_or(TabId::FIRST, TabId::next);
        *last = Some(next);
        next
    }

    pub(crate) fn begin(self: &Arc<Self>, typed: &Typed<'_>, tab: Option<(TabId, Name)>) -> Pending {
        let mut calls = self.activity.lock().unwrap_or_else(PoisonError::into_inner);
        let id = calls.next;
        calls.next += 1;
        let time = humantime::format_rfc3339_millis(typed.time).to_string();
        let (tab_id, tab) = tab.unzip();
        calls.pending.push((
            id,
            PendingCall {
                time,
                tab_id,
                tab,
                command: call_line(typed.words),
                action: typed.action.map(str::to_string),
            },
        ));
        drop(calls);
        self.logged.notify();
        Pending { folder: Arc::clone(self), id }
    }

    pub(crate) fn log(&self, entry: &ActivityEntry) {
        self.append(entry, None);
    }

    fn append(&self, entry: &ActivityEntry, ending: Option<u64>) {
        let path = self.path.join("activity.jsonl");
        let appended = serde_json::to_string(entry).map_err(anyhow::Error::from).and_then(|line| {
            let mut calls = self.activity.lock().unwrap_or_else(PoisonError::into_inner);
            calls.pending.retain(|(id, _)| Some(*id) != ending);
            append_line(&path, &line)
        });
        if let Err(e) = appended {
            log::note(format_args!("adding `{}` to the activity log {} failed: {e:#}", entry.command, path.display()));
        }
        self.logged.notify();
    }

    pub(crate) fn keep_screenshot(&self, entry: &ScreenshotEntry) {
        let path = self.path.join("screenshots.jsonl");
        let appended = serde_json::to_string(entry).map_err(anyhow::Error::from).and_then(|line| {
            let _turn = self.screenshots.lock().unwrap_or_else(PoisonError::into_inner);
            append_line(&path, &line)
        });
        match appended {
            Ok(()) => self.screenshots_listed.notify(),
            Err(e) => log::note(format_args!(
                "adding {} to the screenshot list {} failed: {e:#}",
                entry.path.display(),
                path.display()
            )),
        }
    }

    pub(crate) fn screenshots_listed(&self) -> watch::Receiver<u64> {
        self.screenshots_listed.subscribe()
    }

    pub(crate) fn logged(&self) -> watch::Receiver<u64> {
        self.logged.subscribe()
    }

    pub(crate) fn activity_and_pending_since(
        &self,
        offset: u64,
    ) -> Result<(Vec<serde_json::Value>, u64, Vec<PendingCall>)> {
        let calls = self.activity.lock().unwrap_or_else(PoisonError::into_inner);
        let (entries, end) = self.activity_since(offset)?;
        Ok((entries, end, calls.pending.iter().map(|(_, call)| call.clone()).collect()))
    }

    pub(crate) fn activity_since(&self, offset: u64) -> Result<(Vec<serde_json::Value>, u64)> {
        activity_in(&self.path, offset)
    }

    pub(crate) fn screenshots(&self) -> Result<Vec<ScreenshotEntry>> {
        screenshots_in(&self.path)
    }

    pub(crate) fn command_log(&self, tab: TabId) -> CommandLog {
        CommandLog { path: self.path.join(format!("t{}.commands.jsonl", tab.0)) }
    }
}

fn activity_in(folder: &Path, offset: u64) -> Result<(Vec<serde_json::Value>, u64)> {
    let path = folder.join("activity.jsonl");
    let log = match File::open(&path) {
        Ok(log) => log,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok((Vec::new(), offset)),
        Err(e) => return Err(e).with_context(|| format!("open {}", path.display())),
    };
    let mut read = appended_since(&log, offset).with_context(|| format!("read {}", path.display()))?;
    let complete = read.iter().rposition(|byte| *byte == b'\n').map_or(0, |end| end + 1);
    read.truncate(complete);
    let entries = read
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(serde_json::from_slice)
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("read {}", path.display()))?;
    Ok((entries, offset + complete as u64))
}

fn appended_since(log: &File, offset: u64) -> Result<Vec<u8>> {
    let length = log.metadata()?.len();
    let mut read = vec![0; usize::try_from(length.saturating_sub(offset))?];
    log.read_exact_at(&mut read, offset)?;
    Ok(read)
}

fn screenshots_in(folder: &Path) -> Result<Vec<ScreenshotEntry>> {
    let path = folder.join("screenshots.jsonl");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
    };
    let complete = text.rfind('\n').map_or(0, |end| end + 1);
    let mut kept: Vec<ScreenshotEntry> = Vec::new();
    for line in text[..complete].lines() {
        let entry: ScreenshotEntry =
            serde_json::from_str(line).with_context(|| format!("read {}: {line}", path.display()))?;
        kept.retain(|earlier| earlier.path != entry.path);
        kept.push(entry);
    }
    Ok(kept)
}

#[derive(Clone)]
pub(crate) struct CommandLog {
    path: PathBuf,
}

#[derive(Serialize)]
struct LoggedCommand<'a> {
    #[serde(flatten)]
    record: &'a CommandRecord,
    output: &'a str,
}

impl CommandLog {
    pub(crate) fn append(&self, record: &CommandRecord, output: &str) {
        let appended = serde_json::to_string(&LoggedCommand { record, output })
            .map_err(anyhow::Error::from)
            .and_then(|line| append_line(&self.path, &line));
        if let Err(e) = appended {
            log::note(format_args!(
                "adding command {} `{}` to the command log {} failed: {e:#}",
                record.id,
                record.command,
                self.path.display()
            ));
        }
    }
}

fn append_line(path: &Path, line: &str) -> Result<()> {
    let mut log = OpenOptions::new().create(true).append(true).open(path)?;
    log.write_all(format!("{line}\n").as_bytes())?;
    Ok(())
}

static WRITES: AtomicU64 = AtomicU64::new(0);

pub(crate) fn write_whole(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let name = path.file_name().with_context(|| format!("{} names no file", path.display()))?;
    let write = WRITES.fetch_add(1, Ordering::Relaxed);
    let partial = parent.join(format!(".{}.{}-{write}.partial", name.to_string_lossy(), std::process::id()));
    std::fs::write(&partial, bytes).with_context(|| format!("write {}", partial.display()))?;
    std::fs::rename(&partial, path).with_context(|| format!("move {} to {}", partial.display(), path.display()))
}

pub(crate) struct Exported {
    pub files: usize,
    pub bytes: u64,
    pub missing: Vec<PathBuf>,
}

pub(crate) fn export(folder: &SessionFolder, export: &request::TraceExport) -> Result<Exported> {
    let to = export.path.as_path();
    let mut zip = SessionZip::new(file_name(&export.session));
    let files = zip.add_folder(&folder.path)?;
    let (added, missing) = zip.add_screenshots(&folder.path)?;
    let zipped = zip.writer.finish()?.into_inner();
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(to, &zipped).with_context(|| format!("write {}", to.display()))?;
    Ok(Exported { files: files + added, bytes: zipped.len() as u64, missing })
}

struct SessionZip {
    writer: ZipWriter<Cursor<Vec<u8>>>,
    root: String,
    options: SimpleFileOptions,
    named: Vec<String>,
}

impl SessionZip {
    fn new(root: String) -> Self {
        Self {
            writer: ZipWriter::new(Cursor::new(Vec::new())),
            root,
            options: SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
            named: Vec::new(),
        }
    }

    fn add_folder(&mut self, folder: &Path) -> Result<usize> {
        let mut files = Vec::new();
        files_in(folder, &mut files)?;
        for file in &files {
            let mut contents = std::fs::read(file).with_context(|| format!("read {}", file.display()))?;
            if is_appended_to(file) {
                contents.truncate(contents.iter().rposition(|byte| *byte == b'\n').map_or(0, |end| end + 1));
            }
            let name = file
                .strip_prefix(folder)
                .with_context(|| format!("{} is outside {}", file.display(), folder.display()))?;
            self.writer.start_file(format!("{}/{}", self.root, name.display()), self.options)?;
            self.writer.write_all(&contents)?;
        }
        Ok(files.len())
    }

    fn add_screenshots(&mut self, folder: &Path) -> Result<(usize, Vec<PathBuf>)> {
        let mut missing = Vec::new();
        let mut added = 0;
        for path in saved_screenshots(folder)? {
            match std::fs::read(&path) {
                Ok(contents) => {
                    self.add_screenshot(&path, &contents)?;
                    added += 1;
                }
                Err(e) if e.kind() == ErrorKind::NotFound => missing.push(path),
                Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
            }
        }
        Ok((added, missing))
    }

    fn add_screenshot(&mut self, path: &Path, contents: &[u8]) -> Result<()> {
        let name = path.file_name().map_or_else(|| "file".into(), |name| name.to_string_lossy().into_owned());
        let mut entry = format!("{}/screenshots/{name}", self.root);
        for copy in 2.. {
            if !self.named.contains(&entry) {
                break;
            }
            entry = format!("{}/screenshots/{copy}-{name}", self.root);
        }
        self.writer.start_file(entry.as_str(), self.options)?;
        self.writer.write_all(contents)?;
        self.named.push(entry);
        Ok(())
    }
}

fn saved_screenshots(folder: &Path) -> Result<Vec<PathBuf>> {
    let mut saved: Vec<PathBuf> = Vec::new();
    let mut keep = |path: PathBuf| {
        if !saved.contains(&path) {
            saved.push(path);
        }
    };
    let (calls, _) = activity_in(folder, 0)?;
    for call in &calls {
        for path in call["screenshots"].as_array().into_iter().flatten().filter_map(serde_json::Value::as_str) {
            keep(PathBuf::from(path));
        }
    }
    for shot in screenshots_in(folder)? {
        keep(shot.path);
    }
    Ok(saved)
}

fn is_appended_to(file: &Path) -> bool {
    file.extension().is_some_and(|extension| extension == "jsonl")
}

fn files_in(folder: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = std::fs::read_dir(folder)
        .and_then(|entries| entries.map(|entry| entry.map(|entry| entry.path())).collect::<io::Result<Vec<_>>>())
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

pub(crate) fn folder(home: &Path, session: &str) -> PathBuf {
    home.join("sessions").join(file_name(session))
}

pub(crate) fn file_name(session: &str) -> String {
    if session.is_empty() {
        return "%".into();
    }
    fmt::from_fn(|f| {
        session.bytes().enumerate().try_for_each(|(i, byte)| {
            let kept = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') || (byte == b'.' && i > 0);
            if kept { f.write_char(char::from(byte)) } else { write!(f, "%{byte:02X}") }
        })
    })
    .to_string()
}

#[cfg(test)]
mod tests;
