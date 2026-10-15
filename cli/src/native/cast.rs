use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

use crate::native::screen::Screen;
use crate::protocol::Size;

#[derive(Deserialize)]
pub struct Header {
    pub width: u16,
    pub height: u16,
    pub timestamp: u64,
}

impl Header {
    pub fn size(&self) -> Size {
        Size { cols: self.width, rows: self.height }
    }
}

pub struct Event {
    pub time: f64,
    pub change: Change,
}

pub enum Change {
    Output(String),
    Resize(Size),
    Mark(String),
    Input,
}

impl Change {
    pub fn draws(&self) -> bool {
        matches!(self, Change::Output(_) | Change::Resize(_))
    }

    pub fn apply(&self, screen: &mut Screen) {
        match self {
            Change::Output(text) => screen.draw(text.as_bytes()),
            Change::Resize(size) => screen.resize(*size),
            Change::Mark(_) | Change::Input => {}
        }
    }
}

pub struct Reader {
    path: PathBuf,
    lines: BufReader<File>,
    at: u64,
    header: Header,
}

impl Reader {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("open the recording {}", path.display()))?;
        let mut lines = BufReader::new(file);
        let mut header = String::new();
        let read = lines.read_line(&mut header).with_context(|| format!("read the recording {}", path.display()))?;
        if !header.ends_with('\n') {
            return Err(anyhow!("the recording {} has no header yet", path.display()));
        }
        let header = serde_json::from_str(&header)
            .with_context(|| format!("read the header of the recording {}", path.display()))?;
        Ok(Self { path: path.to_path_buf(), lines, at: read as u64, header })
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn at(&self) -> u64 {
        self.at
    }

    pub fn seek(&mut self, offset: u64) -> Result<()> {
        self.lines
            .seek(SeekFrom::Start(offset))
            .with_context(|| format!("read the recording {}", self.path.display()))?;
        self.at = offset;
        Ok(())
    }

    pub fn next(&mut self) -> Result<Option<Event>> {
        loop {
            let mut line = String::new();
            let read = self
                .lines
                .read_line(&mut line)
                .with_context(|| format!("read the recording {}", self.path.display()))?;
            if !line.ends_with('\n') {
                self.seek(self.at)?;
                return Ok(None);
            }
            self.at += read as u64;
            let (time, kind, data): (f64, String, String) = serde_json::from_str(&line).with_context(|| {
                format!("read an event of the recording {}: {}", self.path.display(), line.trim_end())
            })?;
            let change = match kind.as_str() {
                "o" => Change::Output(data),
                "r" => Change::Resize(size(&data).ok_or_else(|| {
                    anyhow!("the recording {} has a resize to {data:?}, not COLSxROWS", self.path.display())
                })?),
                "m" => Change::Mark(data),
                "i" => Change::Input,
                _ => continue,
            };
            return Ok(Some(Event { time, change }));
        }
    }
}

fn size(text: &str) -> Option<Size> {
    let (cols, rows) = text.split_once('x')?;
    Some(Size { cols: cols.parse().ok()?, rows: rows.parse().ok()? })
}
