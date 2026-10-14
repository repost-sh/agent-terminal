use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

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
        #[serde(default)]
        options: SnapshotOptions,
    },
    Close {
        session: String,
        all: bool,
    },
    Stop,
    Get {
        session: String,
        what: Get,
    },
    Find {
        session: String,
        text: String,
        regex: bool,
        all: bool,
    },
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
pub enum Response {
    Ok(Reply),
    Err(ErrorBody),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Reply {
    Snapshot(Snapshot),
    Closed(Closed),
    Stopped,
    Fact(Fact),
    Matches(Matches),
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
