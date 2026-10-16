use std::io::{Read, Write};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde::de::DeserializeOwned;

const MAX_MESSAGE: u32 = 64 * 1024 * 1024;

pub(crate) fn write<T: Serialize>(stream: &mut impl Write, message: &T) -> Result<()> {
    let body = serde_json::to_vec(message)?;
    let len = u32::try_from(body.len()).context("the message is too large to send")?;
    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(&body)?;
    stream.flush()?;
    Ok(())
}

pub(crate) fn read<T: DeserializeOwned>(stream: &mut impl Read) -> Result<Option<T>> {
    match read_body(stream)? {
        Some(body) => Ok(Some(serde_json::from_slice(&body)?)),
        None => Ok(None),
    }
}

pub(crate) fn read_body(stream: &mut impl Read) -> Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    let first = loop {
        match stream.read(&mut len[..1]) {
            Ok(n) => break n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
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
    Ok(Some(body))
}
