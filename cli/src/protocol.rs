use std::fmt;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::key::Key;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "until", rename_all = "kebab-case")]
pub enum Wait {
    Stable { quiet_ms: u64 },
    Text { text: String },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "command", rename_all = "kebab-case")]
pub enum Request {
    Open {
        session: String,
        program: String,
        args: Vec<String>,
        size: Size,
        cwd: PathBuf,
        env: Vec<(String, String)>,
        wait: Wait,
        timeout_ms: u64,
    },
    Snapshot {
        session: String,
        tab: Option<u32>,
        #[serde(default)]
        options: SnapshotOptions,
    },
    Close {
        session: String,
        all: bool,
        tab: Option<u32>,
    },
    Resize {
        session: String,
        tab: Option<u32>,
        size: Size,
        wait: Wait,
        timeout_ms: u64,
    },
    Restart {
        session: String,
        tab: Option<u32>,
        wait: Wait,
        timeout_ms: u64,
    },
    Sessions,
    Tabs {
        session: String,
    },
    SwitchTab {
        session: String,
        tab: u32,
    },
    Input {
        session: String,
        tab: Option<u32>,
        input: Input,
        wait: Option<Wait>,
        timeout_ms: u64,
    },
    Stop,
    Get {
        session: String,
        tab: Option<u32>,
        what: Get,
    },
    Find {
        session: String,
        tab: Option<u32>,
        text: String,
        regex: bool,
        all: bool,
    },
    Mark {
        session: String,
        tab: Option<u32>,
        name: String,
    },
    Output {
        session: String,
        tab: Option<u32>,
        since: Option<String>,
        raw: bool,
    },
    DiffSnapshot {
        session: String,
        tab: Option<u32>,
        baseline: Option<Baseline>,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Baseline {
    pub name: String,
    pub lines: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct SnapshotOptions {
    pub rows: Option<RowRange>,
    pub scrollback: Scrollback,
    pub ansi: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowRange {
    pub first: u16,
    pub last: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Scrollback {
    #[default]
    None,
    All,
    Last(usize),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum Get {
    Text,
    Cursor,
    Size,
    Frame,
    Program,
    ExitCode,
    Pid,
    Title,
    Cwd,
    Modes,
    Cell { row: u16, col: u16 },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Input {
    Type { text: String, delay_ms: u64 },
    Press { keys: Vec<Key>, repeat: u32 },
    Paste { text: String },
    Send { bytes: Vec<u8> },
    Signal { signal: Signal },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Signal {
    Int,
    Term,
    Kill,
    Hup,
    Tstp,
    Cont,
    Winch,
}

impl Signal {
    const ALL: [Signal; 7] =
        [Signal::Int, Signal::Term, Signal::Kill, Signal::Hup, Signal::Tstp, Signal::Cont, Signal::Winch];

    pub fn name(self) -> &'static str {
        match self {
            Signal::Int => "INT",
            Signal::Term => "TERM",
            Signal::Kill => "KILL",
            Signal::Hup => "HUP",
            Signal::Tstp => "TSTP",
            Signal::Cont => "CONT",
            Signal::Winch => "WINCH",
        }
    }
}

impl FromStr for Signal {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let upper = text.to_ascii_uppercase();
        let name = upper.strip_prefix("SIG").unwrap_or(&upper);
        Signal::ALL
            .into_iter()
            .find(|signal| signal.name() == name)
            .ok_or_else(|| format!("unknown signal {text}; use INT, TERM, KILL, HUP, TSTP, CONT or WINCH"))
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Ok(Reply),
    Err(ErrorBody),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Reply {
    Snapshot(Snapshot),
    Closed(Closed),
    Sessions(Sessions),
    Tabs(SessionInfo),
    Delivered(Delivered),
    Stopped,
    Fact(Fact),
    Matches(Matches),
    Marked(Marked),
    Printed(Printed),
    Diff(SnapshotDiff),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SnapshotDiff {
    pub since: DiffBase,
    pub frame: u64,
    pub changes: Vec<RowChange>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DiffBase {
    Frame(u64),
    Baseline(String),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RowChange {
    pub row: u16,
    pub before: String,
    pub after: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Marked {
    pub mark: String,
    pub frame: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Printed {
    pub output: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Matches {
    pub matches: Vec<Match>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Match {
    pub row: i32,
    pub col: u16,
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Fact {
    Text(String),
    Cursor(Cursor),
    Size(Size),
    Frame(u64),
    Program(String),
    ExitCode(Exit),
    Pid(Option<u32>),
    Title(Option<String>),
    Cwd(String),
    Modes(Modes),
    Cell(Cell),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Cell {
    pub row: u16,
    pub col: u16,
    pub char: String,
    pub fg: String,
    pub bg: String,
    pub bold: bool,
    pub inverse: bool,
    pub underline: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Modes {
    pub alt_screen: bool,
    pub mouse: Option<Mouse>,
    pub bracketed_paste: bool,
    pub app_cursor: bool,
    pub focus_events: bool,
    pub keyboard: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Mouse {
    pub tracking: String,
    pub encoding: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Exit {
    pub exited: bool,
    pub code: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Cursor {
    pub row: u16,
    pub col: u16,
    pub visible: bool,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Sessions {
    pub sessions: Vec<SessionInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SessionInfo {
    pub session: String,
    pub current: Option<String>,
    pub tabs: Vec<TabInfo>,
}

impl SessionInfo {
    pub fn empty(session: &str) -> Self {
        Self { session: session.to_string(), current: None, tabs: Vec::new() }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TabInfo {
    pub tab: String,
    pub program: String,
    pub cols: u16,
    pub rows: u16,
    pub exited: bool,
    pub exit_code: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorBody {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<Box<Snapshot>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Snapshot {
    pub session: String,
    pub tab: String,
    pub program: String,
    pub cols: u16,
    pub rows: u16,
    pub alt_screen: bool,
    pub mouse: Option<String>,
    pub cursor: [u16; 2],
    pub cursor_visible: bool,
    pub frame: u64,
    pub exited: bool,
    pub exit_code: Option<i32>,
    pub first_row: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrollback: Option<Vec<String>>,
    pub lines: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Closed {
    pub closed: Vec<TabRef>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Delivered {
    pub session: String,
    pub tab: String,
    #[serde(flatten)]
    pub delivery: Delivery,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "input", rename_all = "kebab-case")]
pub enum Delivery {
    Text { keys: usize, bytes: usize },
    Keys { keys: usize, bytes: usize },
    Paste { bracketed: bool, bytes: usize },
    Raw { bytes: usize },
    Signal { signal: Signal, process_group: i32 },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TabRef {
    pub session: String,
    pub tab: String,
}

const MAX_MESSAGE: u32 = 64 * 1024 * 1024;

pub fn write<T: Serialize>(stream: &mut impl Write, message: &T) -> Result<()> {
    let body = serde_json::to_vec(message)?;
    stream.write_all(&(body.len() as u32).to_be_bytes())?;
    stream.write_all(&body)?;
    stream.flush()?;
    Ok(())
}

pub fn read<T: DeserializeOwned>(stream: &mut impl Read) -> Result<Option<T>> {
    let mut len = [0u8; 4];
    let first = loop {
        match stream.read(&mut len[..1]) {
            Ok(n) => break n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e).context("read message length"),
        }
    };
    if first == 0 {
        return Ok(None);
    }
    stream.read_exact(&mut len[1..]).context("read message length")?;
    let len = u32::from_be_bytes(len);
    if len > MAX_MESSAGE {
        bail!("message too large: {len} bytes");
    }
    let mut body = vec![0u8; len as usize];
    stream.read_exact(&mut body).context("read message")?;
    Ok(Some(serde_json::from_slice(&body)?))
}
