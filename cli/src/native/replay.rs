use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

use crate::native::screen::Screen;
use crate::native::terminal::{Picture, Run};
use crate::protocol::Size;

pub struct Replay {
    cast: PathBuf,
    reader: BufReader<File>,
    at: u64,
    timestamp: u64,
    screen: Screen,
    drawn_at: f64,
    frame: u64,
}

pub struct Drawn {
    pub picture: Picture,
    pub time_ms: u64,
}

#[derive(Deserialize)]
struct Header {
    timestamp: u64,
}

impl Replay {
    pub fn start(run: &Run) -> Result<Self> {
        let cast = run.cast.clone();
        let file = File::open(&cast).with_context(|| format!("open the recording {}", cast.display()))?;
        let mut reader = BufReader::new(file);
        let mut header = String::new();
        reader.read_line(&mut header).with_context(|| format!("read the recording {}", cast.display()))?;
        let header: Header =
            serde_json::from_str(&header).with_context(|| format!("read the header of {}", cast.display()))?;
        reader.seek(SeekFrom::Start(run.from)).with_context(|| format!("read the recording {}", cast.display()))?;
        Ok(Self {
            cast,
            reader,
            at: run.from,
            timestamp: header.timestamp,
            screen: Screen::new(run.size),
            drawn_at: 0.0,
            frame: 0,
        })
    }

    pub fn next_frame(&mut self, end: u64) -> Result<Drawn> {
        let frame = self.frame + 1;
        if end <= self.at {
            bail!("{} holds nothing that drew frame {frame}", self.cast.display());
        }
        while self.at < end {
            let mut line = String::new();
            let read = self.reader.read_line(&mut line).with_context(|| format!("read {}", self.cast.display()))?;
            if read == 0 {
                bail!("{} ends before frame {frame}", self.cast.display());
            }
            self.at += read as u64;
            let (time, kind, data): (f64, String, String) = serde_json::from_str(&line)
                .with_context(|| format!("read an event of {} before frame {frame}", self.cast.display()))?;
            match kind.as_str() {
                "o" => self.screen.draw(data.as_bytes()),
                "r" => self.screen.resize(size(&data)?),
                _ => continue,
            }
            self.drawn_at = time;
        }
        if self.at != end {
            bail!("{} has no event ending where frame {frame} was drawn", self.cast.display());
        }
        self.frame = frame;
        Ok(Drawn {
            picture: Picture::of(&self.screen, frame, None),
            time_ms: self.timestamp * 1000 + (self.drawn_at * 1000.0).round() as u64,
        })
    }
}

fn size(data: &str) -> Result<Size> {
    let unreadable = || anyhow!("{data:?} is not a COLSxROWS size");
    let (cols, rows) = data.split_once('x').ok_or_else(unreadable)?;
    Ok(Size { cols: cols.parse().map_err(|_| unreadable())?, rows: rows.parse().map_err(|_| unreadable())? })
}
