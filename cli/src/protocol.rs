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
    },
    Close {
        session: String,
        all: bool,
    },
    Input {
        session: String,
        input: Input,
        wait: Option<Wait>,
        timeout_ms: u64,
    },
    Stop,
    Batch {
        steps: Vec<BatchStep>,
        bail: bool,
    },
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
#[serde(tag = "reply", content = "data")]
pub enum Reply {
    Snapshot(Snapshot),
    Closed(Closed),
    Delivered(Delivered),
    Stopped,
    Batch(Vec<BatchOutcome>),
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
