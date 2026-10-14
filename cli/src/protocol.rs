use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "command", rename_all = "kebab-case")]
pub enum Request {
    Open {
        session: String,
        program: String,
        args: Vec<String>,
        cols: u16,
        rows: u16,
        cwd: PathBuf,
        env: Vec<(String, String)>,
        wait_stable_ms: u64,
        timeout_ms: u64,
    },
    Snapshot {
        session: String,
    },
    Close {
        session: String,
        all: bool,
    },
    Stop,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum Response {
    Ok { ok: serde_json::Value },
    Err { error: ErrorBody },
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

pub fn read<T: DeserializeOwned>(stream: &mut impl Read) -> Result<T> {
    let mut len = [0u8; 4];
    stream.read_exact(&mut len).context("read message length")?;
    let len = u32::from_be_bytes(len);
    if len > MAX_MESSAGE {
        bail!("message too large: {len} bytes");
    }
    let mut body = vec![0u8; len as usize];
    stream.read_exact(&mut body).context("read message")?;
    Ok(serde_json::from_slice(&body)?)
}
