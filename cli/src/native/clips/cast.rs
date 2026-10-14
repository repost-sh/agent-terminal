use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

use crate::protocol::Size;

pub struct Cast {
    pub size: Size,
    pub timestamp: u64,
    pub events: Vec<Event>,
}

pub struct Event {
    pub time: f64,
    pub change: Change,
}

pub enum Change {
    Output(String),
    Resize(Size),
    Mark(String),
}

#[derive(Deserialize)]
struct Header {
    width: u16,
    height: u16,
    timestamp: u64,
}

pub fn read(path: &Path, until: f64) -> Result<Cast> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read the recording {}", path.display()))?;
    let complete = &text[..text.rfind('\n').map_or(0, |end| end + 1)];
    let mut lines = complete.lines();
    let header = lines.next().ok_or_else(|| anyhow!("the recording {} has no header yet", path.display()))?;
    let header: Header =
        serde_json::from_str(header).with_context(|| format!("read the header of the recording {}", path.display()))?;
    let mut events = Vec::new();
    for line in lines {
        let (time, kind, data): (f64, String, String) =
            serde_json::from_str(line).with_context(|| format!("read the recording {}", path.display()))?;
        if time > until {
            break;
        }
        let change =
            match kind.as_str() {
                "o" => Change::Output(data),
                "r" => Change::Resize(size(&data).ok_or_else(|| {
                    anyhow!("the recording {} has a resize to {data:?}, not COLSxROWS", path.display())
                })?),
                "m" => Change::Mark(data),
                _ => continue,
            };
        events.push(Event { time, change });
    }
    Ok(Cast { size: Size { cols: header.width, rows: header.height }, timestamp: header.timestamp, events })
}

fn size(text: &str) -> Option<Size> {
    let (cols, rows) = text.split_once('x')?;
    Some(Size { cols: cols.parse().ok()?, rows: rows.parse().ok()? })
}
