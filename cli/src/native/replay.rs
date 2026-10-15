use anyhow::{Result, bail};

use crate::native::cast::Reader;
use crate::native::screen::Screen;
use crate::native::terminal::{Picture, Run};

pub struct Replay {
    cast: Reader,
    screen: Screen,
    drawn_at: f64,
    frame: u64,
}

pub struct Drawn {
    pub picture: Picture,
    pub time_ms: u64,
}

impl Replay {
    pub fn start(run: &Run) -> Result<Self> {
        let mut cast = Reader::open(&run.cast)?;
        cast.seek(run.from)?;
        Ok(Self { cast, screen: Screen::new(run.size), drawn_at: 0.0, frame: 0 })
    }

    pub fn next_frame(&mut self, end: u64) -> Result<Drawn> {
        let frame = self.frame + 1;
        let cast = self.cast.path().display().to_string();
        if end <= self.cast.at() {
            bail!("{cast} holds nothing that drew frame {frame}");
        }
        while self.cast.at() < end {
            let Some(event) = self.cast.next()? else {
                bail!("{cast} ends before frame {frame}");
            };
            event.change.apply(&mut self.screen);
            if event.change.draws() {
                self.drawn_at = event.time;
            }
        }
        if self.cast.at() != end {
            bail!("{cast} has no event ending where frame {frame} was drawn");
        }
        self.frame = frame;
        Ok(Drawn {
            picture: Picture::of(&self.screen, frame, None),
            time_ms: self.cast.header().timestamp * 1000 + (self.drawn_at * 1000.0).round() as u64,
        })
    }
}
