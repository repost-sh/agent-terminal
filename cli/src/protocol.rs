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

impl Size {
    pub fn new(cols: u16, rows: u16) -> Result<Self, String> {
        if !(2..=1000).contains(&cols) || !(2..=1000).contains(&rows) {
            return Err("columns and rows must be between 2 and 1000".into());
        }
        Ok(Self { cols, rows })
    }
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
    Command,
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(try_from = "String", into = "String")]
pub struct TabId(u32);

impl TabId {
    pub const FIRST: TabId = TabId(1);

    pub fn next(self) -> TabId {
        TabId(self.0 + 1)
    }
}

impl FromStr for TabId {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        text.strip_prefix('t')
            .unwrap_or(text)
            .parse()
            .ok()
            .filter(|id| *id > 0)
            .map(TabId)
            .ok_or_else(|| format!("invalid tab id: {text} (expected t1, t2, ...)"))
    }
}

impl fmt::Display for TabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0)
    }
}

impl TryFrom<String> for TabId {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        text.parse()
    }
}

impl From<TabId> for String {
    fn from(id: TabId) -> String {
        id.to_string()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub session: String,
    pub tab: Option<TabId>,
}

impl Target {
    pub fn tab(session: &str, tab: TabId) -> Self {
        Self { session: session.to_string(), tab: Some(tab) }
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
    Tab(&'a str, Option<TabId>),
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
        #[serde(flatten)]
        target: Target,
        #[serde(default)]
        options: SnapshotOptions,
    },
    Close {
        #[serde(flatten)]
        target: Target,
        all: bool,
    },
    Resize {
        #[serde(flatten)]
        target: Target,
        size: Size,
        wait: Wait,
        timeout_ms: u64,
    },
    Restart {
        #[serde(flatten)]
        target: Target,
        wait: Wait,
        timeout_ms: u64,
    },
    Sessions,
    Tabs {
        session: String,
    },
    SwitchTab {
        session: String,
        tab: TabId,
    },
    Input {
        #[serde(flatten)]
        target: Target,
        input: Input,
        wait: Option<Wait>,
        #[serde(default)]
        snapshot: bool,
        #[serde(default)]
        screenshot: Option<Shot>,
        timeout_ms: u64,
    },
    Wait {
        #[serde(flatten)]
        target: Target,
        wait: Wait,
        timeout_ms: u64,
        #[serde(default)]
        assert: bool,
    },
    Expect {
        #[serde(flatten)]
        target: Target,
        expect: Expect,
    },
    Stop,
    Batch {
        steps: Vec<BatchStep>,
        bail: bool,
    },
    Get {
        #[serde(flatten)]
        target: Target,
        what: Get,
    },
    Find {
        #[serde(flatten)]
        target: Target,
        text: String,
        regex: bool,
        all: bool,
    },
    Mark {
        #[serde(flatten)]
        target: Target,
        name: String,
    },
    Output {
        #[serde(flatten)]
        target: Target,
        since: Option<String>,
        raw: bool,
    },
    DiffSnapshot {
        #[serde(flatten)]
        target: Target,
        baseline: Option<Baseline>,
    },
    TraceExport {
        session: String,
        path: PathBuf,
    },
    DashboardStart {
        port: u16,
        #[serde(default)]
        origins: Vec<String>,
    },
    DashboardStop,
    Shell {
        session: String,
        shell: Shell,
        program: String,
        size: Size,
        cwd: PathBuf,
        env: Vec<(String, String)>,
        wait: Option<Wait>,
        timeout_ms: u64,
    },
    Exec {
        #[serde(flatten)]
        target: Target,
        line: String,
        check: bool,
        timeout_ms: u64,
    },
    History {
        #[serde(flatten)]
        target: Target,
        last: Option<usize>,
        failed: bool,
    },
    HistoryShow {
        #[serde(flatten)]
        target: Target,
        id: u32,
    },
    Screenshot {
        #[serde(flatten)]
        target: Target,
        shot: Shot,
    },
    DiffScreenshot {
        #[serde(flatten)]
        target: Target,
        baseline: PathBuf,
        shot: Shot,
        threshold: f64,
    },
    WaitControl {
        #[serde(flatten)]
        target: Target,
        timeout_ms: u64,
        #[serde(default)]
        assert: bool,
    },
    Attach {
        #[serde(flatten)]
        target: Target,
        read_only: bool,
    },
}

impl Request {
    pub fn addressed(&self) -> Addressed<'_> {
        match self {
            Request::Sessions
            | Request::Stop
            | Request::Batch { .. }
            | Request::DashboardStart { .. }
            | Request::DashboardStop
            | Request::Attach { .. } => Addressed::Nothing,
            Request::Open { session, .. }
            | Request::Shell { session, .. }
            | Request::Tabs { session }
            | Request::Close { target: Target { session, .. }, all: true }
            | Request::TraceExport { session, .. } => Addressed::Session(session),
            Request::SwitchTab { session, tab } => Addressed::Tab(session, Some(*tab)),
            Request::Snapshot { target, .. }
            | Request::Close { target, .. }
            | Request::Resize { target, .. }
            | Request::Restart { target, .. }
            | Request::Input { target, .. }
            | Request::Wait { target, .. }
            | Request::Expect { target, .. }
            | Request::Get { target, .. }
            | Request::Find { target, .. }
            | Request::Mark { target, .. }
            | Request::Output { target, .. }
            | Request::DiffSnapshot { target, .. }
            | Request::Exec { target, .. }
            | Request::History { target, .. }
            | Request::HistoryShow { target, .. }
            | Request::Screenshot { target, .. }
            | Request::DiffScreenshot { target, .. }
            | Request::WaitControl { target, .. } => Addressed::Tab(&target.session, target.tab),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Owner {
    Agent,
    Human,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum HumanInput {
    Typed(String),
    Pressed(Key),
    Pasted(String),
}

impl HumanInput {
    pub fn input(&self) -> Input {
        match self {
            HumanInput::Typed(text) => Input::Type { text: text.clone(), delay_ms: 0 },
            HumanInput::Pressed(key) => Input::Press { keys: vec![*key], repeat: 1 },
            HumanInput::Pasted(text) => Input::Paste { text: text.clone() },
        }
    }

    pub fn call(&self) -> Vec<String> {
        match self {
            HumanInput::Typed(text) => vec!["type".into(), text.clone()],
            HumanInput::Pressed(key) => vec!["press".into(), key.to_string()],
            HumanInput::Pasted(text) => vec!["paste".into(), text.clone()],
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum AttachInput {
    Human(HumanInput),
    Detach,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum AttachEvent {
    Screen(Mirror),
    Detached,
    Closed,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Mirror {
    pub program: String,
    pub frame: u64,
    pub cols: u16,
    pub rows: u16,
    pub lines: Vec<String>,
    pub cursor: [u16; 2],
    pub cursor_visible: bool,
    pub exited: bool,
    pub exit_code: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Shell {
    Zsh,
    Bash,
    Fish,
}

impl Shell {
    pub const ALL: [Shell; 3] = [Shell::Zsh, Shell::Bash, Shell::Fish];

    pub fn name(self) -> &'static str {
        match self {
            Shell::Zsh => "zsh",
            Shell::Bash => "bash",
            Shell::Fish => "fish",
        }
    }

    pub fn of_program(program: &str) -> Option<Shell> {
        let name = std::path::Path::new(program).file_name()?.to_str()?;
        let name = name.strip_prefix('-').unwrap_or(name);
        Shell::ALL.into_iter().find(|shell| shell.name() == name)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl FromStr for Theme {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        match text.to_ascii_lowercase().as_str() {
            "dark" => Ok(Theme::Dark),
            "light" => Ok(Theme::Light),
            _ => Err(format!("unknown theme {text}; use dark or light")),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ScreenshotFormat {
    #[default]
    Png,
    Svg,
    Html,
    Txt,
    Ansi,
}

impl ScreenshotFormat {
    pub const ALL: [ScreenshotFormat; 5] = [
        ScreenshotFormat::Png,
        ScreenshotFormat::Svg,
        ScreenshotFormat::Html,
        ScreenshotFormat::Txt,
        ScreenshotFormat::Ansi,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ScreenshotFormat::Png => "png",
            ScreenshotFormat::Svg => "svg",
            ScreenshotFormat::Html => "html",
            ScreenshotFormat::Txt => "txt",
            ScreenshotFormat::Ansi => "ansi",
        }
    }
}

impl FromStr for ScreenshotFormat {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let lower = text.to_ascii_lowercase();
        ScreenshotFormat::ALL
            .into_iter()
            .find(|format| format.name() == lower)
            .ok_or_else(|| format!("unknown format {text}; use png, svg, html, txt or ansi"))
    }
}

impl fmt::Display for ScreenshotFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Look {
    pub scale: u8,
    pub theme: Theme,
    pub cursor: bool,
}

impl Default for Look {
    fn default() -> Self {
        Self { scale: 1, theme: Theme::Dark, cursor: true }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Shot {
    pub path: Option<PathBuf>,
    pub dir: Option<PathBuf>,
    pub format: ScreenshotFormat,
    pub look: Look,
    pub rows: Option<RowRange>,
    pub annotate: bool,
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
    Exported(Export),
    Dashboard(Dashboard),
    DashboardStopped(DashboardStopped),
    History(History),
    Command(CommandOutput),
    Screenshot(Screenshot),
    ScreenshotDiff(ScreenshotDiff),
    Attached(Attached),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Attached {
    pub session: String,
    pub tab: String,
    pub program: String,
    pub read_only: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ScreenshotDiff {
    pub session: String,
    pub tab: String,
    pub baseline: PathBuf,
    pub path: PathBuf,
    pub frame: u64,
    pub changed: u64,
    pub pixels: u64,
    pub threshold: f64,
    pub passed: bool,
}

impl ScreenshotDiff {
    pub fn summary(&self) -> String {
        let share = if self.pixels == 0 { 0.0 } else { self.changed as f64 * 100.0 / self.pixels as f64 };
        let verdict = if self.passed { "within" } else { "above" };
        format!(
            "{} of {} pixels ({share:.2}%) differ from {}, {verdict} the threshold {}; diff image {}",
            self.changed,
            self.pixels,
            self.baseline.display(),
            self.threshold,
            self.path.display()
        )
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Screenshot {
    pub session: String,
    pub tab: String,
    pub path: PathBuf,
    pub format: ScreenshotFormat,
    pub frame: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refs: Option<Vec<Ref>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Dashboard {
    pub url: String,
    pub port: u16,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DashboardStopped {
    pub stopped: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Export {
    pub session: String,
    pub path: PathBuf,
    pub files: usize,
    pub bytes: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct History {
    pub session: String,
    pub tab: String,
    pub total: usize,
    pub commands: Vec<CommandRecord>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CommandOutput {
    pub session: String,
    pub tab: String,
    #[serde(flatten)]
    pub record: CommandRecord,
    pub output: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub check: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommandRecord {
    pub id: u32,
    pub command: String,
    pub cwd: String,
    pub started_at_ms: u64,
    pub ended_at_ms: Option<u64>,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub first_frame: u64,
    pub last_frame: Option<u64>,
}

impl CommandRecord {
    pub fn finished(&self) -> bool {
        self.ended_at_ms.is_some()
    }

    pub fn failed(&self) -> bool {
        self.finished() && self.exit_code.is_some_and(|code| code != 0)
    }
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
    pub command: String,
    pub cwd: String,
    pub cols: u16,
    pub rows: u16,
    pub exited: bool,
    pub exit_code: Option<i32>,
    pub control: Owner,
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
    pub screenshot: Option<Box<Screenshot>>,
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
            Reply::Command(ran) if ran.check => ran.record.exit_code.unwrap_or(crate::exit::ERROR),
            Reply::ScreenshotDiff(diff) if !diff.passed => crate::exit::ASSERTION,
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
