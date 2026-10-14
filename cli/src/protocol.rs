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
        tab: Option<u32>,
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
    Stop,
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
    Stopped,
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
