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
    Idle { quiet_ms: u64 },
    Text(TextMatch),
    Gone(TextMatch),
    Exit { code: Option<i32> },
    Cursor { row: u16, col: u16 },
    Frame { frame: u64 },
    Elapsed { ms: u64 },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TextMatch {
    pub text: String,
    pub regex: bool,
    pub rows: Option<RowRange>,
}

impl TextMatch {
    pub fn plain(text: String) -> Self {
        Self { text, regex: false, rows: None }
    }
}

impl fmt::Display for TextMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.regex { write!(f, "/{}/", self.text) } else { write!(f, "\"{}\"", self.text) }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "check", rename_all = "kebab-case")]
pub enum Expect {
    Text(TextMatch),
    Gone(TextMatch),
    ExitCode { code: i32 },
    Cell { row: u16, col: u16, style: Vec<Style> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Style {
    Bold,
    Inverse,
    Underline,
    Fg(String),
    Bg(String),
}

impl Style {
    pub fn holds(&self, cell: &Cell) -> bool {
        match self {
            Style::Bold => cell.bold,
            Style::Inverse => cell.inverse,
            Style::Underline => cell.underline,
            Style::Fg(color) => cell.fg == *color,
            Style::Bg(color) => cell.bg == *color,
        }
    }
}

impl FromStr for Style {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let lower = text.to_ascii_lowercase();
        match lower.split_once('=') {
            None if lower == "bold" => Ok(Style::Bold),
            None if lower == "inverse" => Ok(Style::Inverse),
            None if lower == "underline" => Ok(Style::Underline),
            Some(("fg", color)) if !color.is_empty() => Ok(Style::Fg(color.to_string())),
            Some(("bg", color)) if !color.is_empty() => Ok(Style::Bg(color.to_string())),
            _ => Err(format!("unknown style {text}; use bold, inverse, underline, fg=<color> or bg=<color>")),
        }
    }
}

impl fmt::Display for Style {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Style::Bold => f.write_str("bold"),
            Style::Inverse => f.write_str("inverse"),
            Style::Underline => f.write_str("underline"),
            Style::Fg(color) => write!(f, "fg={color}"),
            Style::Bg(color) => write!(f, "bg={color}"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Call {
    pub typed: Option<Vec<String>>,
    pub request: Request,
}

pub enum Addressed<'a> {
    Nothing,
    Session(&'a str),
    Tab(&'a str, Option<u32>),
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
        wait: Option<Wait>,
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
        #[serde(default)]
        snapshot: bool,
        timeout_ms: u64,
    },
    Wait {
        session: String,
        tab: Option<u32>,
        wait: Wait,
        timeout_ms: u64,
    },
    Expect {
        session: String,
        tab: Option<u32>,
        expect: Expect,
    },
    Stop,
    Batch {
        steps: Vec<BatchStep>,
        bail: bool,
    },
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
    RecordStart {
        session: String,
        tab: Option<u32>,
        path: Option<PathBuf>,
    },
    RecordStop {
        session: String,
        tab: Option<u32>,
    },
    TraceExport {
        session: String,
        path: PathBuf,
    },
}

impl Request {
    pub fn addressed(&self) -> Addressed<'_> {
        match self {
            Request::Sessions | Request::Stop | Request::Batch { .. } => Addressed::Nothing,
            Request::Open { session, .. }
            | Request::Tabs { session }
            | Request::Close { session, all: true, .. }
            | Request::TraceExport { session, .. } => Addressed::Session(session),
            Request::SwitchTab { session, tab } => Addressed::Tab(session, Some(*tab)),
            Request::Snapshot { session, tab, .. }
            | Request::Close { session, tab, .. }
            | Request::Resize { session, tab, .. }
            | Request::Restart { session, tab, .. }
            | Request::Input { session, tab, .. }
            | Request::Wait { session, tab, .. }
            | Request::Expect { session, tab, .. }
            | Request::Get { session, tab, .. }
            | Request::Find { session, tab, .. }
            | Request::Mark { session, tab, .. }
            | Request::Output { session, tab, .. }
            | Request::DiffSnapshot { session, tab, .. }
            | Request::RecordStart { session, tab, .. }
            | Request::RecordStop { session, tab } => Addressed::Tab(session, *tab),
        }
    }
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
    #[serde(default)]
    pub refs: bool,
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
    Click { target: ClickTarget, button: MouseButton, double: bool },
    Mouse { event: MouseEvent },
    Scroll { direction: Direction, count: u32, keys: bool },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(try_from = "String", into = "String")]
pub enum ClickTarget {
    Ref(u32),
    Cell { row: u16, col: u16 },
    Text(String),
}

impl FromStr for ClickTarget {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        if let Some(id) = text.strip_prefix("@t").and_then(|id| id.parse().ok()).filter(|&id: &u32| id > 0) {
            return Ok(ClickTarget::Ref(id));
        }
        let cell = text.split_once(',').and_then(|(row, col)| Some((row.parse().ok()?, col.parse().ok()?)));
        match cell {
            Some((0, _) | (_, 0)) => Err("rows and columns count from 1".into()),
            Some((row, col)) => Ok(ClickTarget::Cell { row, col }),
            None if text.is_empty() => Err("give a ref (@t3), a cell (ROW,COL) or the text to click".into()),
            None => Ok(ClickTarget::Text(text.to_string())),
        }
    }
}

impl fmt::Display for ClickTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClickTarget::Ref(id) => write!(f, "@t{id}"),
            ClickTarget::Cell { row, col } => write!(f, "{row},{col}"),
            ClickTarget::Text(text) => f.write_str(text),
        }
    }
}

impl TryFrom<String> for ClickTarget {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        text.parse()
    }
}

impl From<ClickTarget> for String {
    fn from(target: ClickTarget) -> String {
        target.to_string()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

impl fmt::Display for MouseButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            MouseButton::Left => "left",
            MouseButton::Middle => "middle",
            MouseButton::Right => "right",
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Direction {
    Up,
    Down,
}

impl FromStr for Direction {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        match text.to_ascii_lowercase().as_str() {
            "up" => Ok(Direction::Up),
            "down" => Ok(Direction::Down),
            _ => Err(format!("unknown direction {text}; use up or down")),
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Direction::Up => "up",
            Direction::Down => "down",
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum MouseEvent {
    Move { row: u16, col: u16 },
    Down { row: u16, col: u16, button: MouseButton },
    Up { row: u16, col: u16, button: MouseButton },
    Wheel { row: u16, col: u16, direction: Direction },
}

impl MouseEvent {
    pub fn at(self) -> (u16, u16) {
        match self {
            MouseEvent::Move { row, col }
            | MouseEvent::Down { row, col, .. }
            | MouseEvent::Up { row, col, .. }
            | MouseEvent::Wheel { row, col, .. } => (row, col),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ScrollBy {
    Wheel,
    Keys,
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
#[serde(tag = "reply", content = "data")]
pub enum Reply {
    Snapshot(Snapshot),
    Closed(Closed),
    Sessions(Sessions),
    Tabs(SessionInfo),
    Delivered(Delivered),
    Waited(Waited),
    Checked(Checked),
    Stopped,
    Batch(Vec<BatchOutcome>),
    Fact(Fact),
    Matches(Matches),
    Marked(Marked),
    Printed(Printed),
    Diff(SnapshotDiff),
    Recording(RecordingFile),
    Saved(SavedRecording),
    Exported(Export),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Export {
    pub session: String,
    pub path: PathBuf,
    pub files: usize,
    pub bytes: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RecordingFile {
    pub session: String,
    pub tab: String,
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SavedRecording {
    pub session: String,
    pub tab: String,
    pub path: PathBuf,
    pub duration_ms: u64,
    pub bytes: u64,
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

impl fmt::Display for Cell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} fg={} bg={}", self.char, self.fg, self.bg)?;
        for (on, name) in [(self.bold, "bold"), (self.inverse, "inverse"), (self.underline, "underline")] {
            if on {
                write!(f, " {name}")?;
            }
        }
        Ok(())
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refs: Option<Vec<Ref>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Ref {
    #[serde(rename = "ref")]
    pub id: String,
    pub kind: RefKind,
    pub label: String,
    pub row: u16,
    pub col: u16,
    pub rows: [u16; 2],
    pub cols: [u16; 2],
    pub frame: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum RefKind {
    Panel,
    Selected,
    Button,
    Link,
    Hint,
    Input,
}

impl fmt::Display for RefKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RefKind::Panel => "panel",
            RefKind::Selected => "selected",
            RefKind::Button => "button",
            RefKind::Link => "link",
            RefKind::Hint => "hint",
            RefKind::Input => "input",
        })
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<Box<Snapshot>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "input", rename_all = "kebab-case")]
pub enum Delivery {
    Text {
        keys: usize,
        bytes: usize,
    },
    Keys {
        keys: usize,
        bytes: usize,
    },
    Paste {
        bracketed: bool,
        bytes: usize,
    },
    Raw {
        bytes: usize,
    },
    Signal {
        signal: Signal,
        process_group: i32,
    },
    Click {
        target: ClickTarget,
        row: u16,
        col: u16,
        button: MouseButton,
        double: bool,
        bytes: usize,
    },
    Mouse {
        #[serde(flatten)]
        event: MouseEvent,
        bytes: usize,
    },
    Scroll {
        direction: Direction,
        count: u32,
        by: ScrollBy,
        bytes: usize,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Waited {
    pub session: String,
    pub tab: String,
    pub until: String,
    pub waited_ms: u64,
    pub frame: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Checked {
    pub session: String,
    pub tab: String,
    pub fact: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TabRef {
    pub session: String,
    pub tab: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BatchStep {
    pub args: Vec<String>,
    pub request: Request,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BatchOutcome {
    pub args: Vec<String>,
    pub response: Option<Response>,
}

impl Response {
    pub fn exit_code(&self) -> i32 {
        match self {
            Response::Ok(reply) => reply.exit_code(),
            Response::Err(error) => error.code,
        }
    }
}

impl Reply {
    pub fn exit_code(&self) -> i32 {
        match self {
            Reply::Batch(outcomes) => outcomes
                .iter()
                .filter_map(|outcome| outcome.response.as_ref())
                .map(Response::exit_code)
                .find(|&code| code != 0)
                .unwrap_or(0),
            _ => 0,
        }
    }
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
