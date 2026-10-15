mod gif;
mod mp4;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

use crate::native::cast::{Change, Event, Reader};
use crate::native::refs::Grid;
use crate::native::render::{self, Image};
use crate::native::screen::Screen;
use crate::native::screenshots;
use crate::native::store::{self, SessionFolder};
use crate::native::terminal::Terminal;
use crate::protocol::{Clip, ClipFormat, ClipPoint, ClipRange, ClipSpec, CommandRecord, Look, RowRange, Size};

const SLOT_MS: u64 = 100;
const TRUNCATED: Duration = Duration::from_millis(1);

trait Movie {
    fn add(&mut self, image: &Image, start_ms: u64, end_ms: u64) -> Result<()>;
    fn size(&self) -> (u32, u32);
    fn finish(self) -> Result<Vec<u8>>;
}

struct Filmed {
    bytes: Vec<u8>,
    frames: usize,
    width: u32,
    height: u32,
}

struct Shown {
    look: (u64, Size),
    image: Image,
    start_ms: u64,
}

pub fn cut(session: &str, tab: &str, terminal: &Terminal, folder: &SessionFolder, spec: &ClipSpec) -> Result<Clip> {
    let frame = terminal.frame();
    let position = terminal.recording().position()?;
    let cast = Cast::read(&position.path, position.seconds)?;
    let timeline = Timeline { cast: &cast, now: position.seconds, session, tab, terminal, folder };
    let (from, to) = timeline.span(&spec.range)?;
    let duration_ms = ((to - from) * 1000.0).round() as u64;
    if duration_ms == 0 {
        bail!("the clip would be empty: it starts and ends at the same moment of the recording");
    }
    let filmed = match spec.format {
        ClipFormat::Gif => film(&cast, from, to, duration_ms, gif::Gif::default())?,
        ClipFormat::Mp4 => film(&cast, from, to, duration_ms, mp4::Mp4::default())?,
    };
    let path = match &spec.path {
        Some(path) => path.clone(),
        None => screenshots::folder(spec.dir.as_deref())?.join(format!(
            "{}-{tab}-{frame}.{}",
            store::file_name(session),
            spec.format
        )),
    };
    store::write_whole(&path, &filmed.bytes)?;
    Ok(Clip {
        session: session.to_string(),
        tab: tab.to_string(),
        path,
        format: spec.format,
        frames: filmed.frames,
        duration_ms,
        width: filmed.width,
        height: filmed.height,
    })
}

struct Timeline<'a> {
    cast: &'a Cast,
    now: f64,
    session: &'a str,
    tab: &'a str,
    terminal: &'a Terminal,
    folder: &'a SessionFolder,
}

impl Timeline<'_> {
    fn span(&self, range: &ClipRange) -> Result<(f64, f64)> {
        let (from, to) = match range {
            ClipRange::Last { ms } => (self.now - *ms as f64 / 1000.0, self.now),
            ClipRange::Between { from: start, to: end } => {
                let from = self.moment(start, Edge::Start)?;
                let to = match end {
                    Some(end) => self.moment(end, Edge::End)?,
                    None => self.now,
                };
                if to < from {
                    let end = end.as_ref().map_or_else(|| "now".to_string(), ToString::to_string);
                    bail!("the clip would end before it starts: {end} comes before {start}");
                }
                (from, to)
            }
            ClipRange::Command { id } => {
                let command = self.command(*id)?;
                let from = self.seconds(UNIX_EPOCH + Duration::from_millis(command.started_at_ms))?;
                let to = match command.ended_at_ms {
                    Some(ended) => self.seconds(UNIX_EPOCH + Duration::from_millis(ended) + TRUNCATED)?,
                    None => self.now,
                };
                (from, to)
            }
        };
        Ok((from.max(0.0), to.min(self.now)))
    }

    fn moment(&self, point: &ClipPoint, edge: Edge) -> Result<f64> {
        match point {
            ClipPoint::Mark(name) => self
                .cast
                .events
                .iter()
                .rev()
                .find_map(|event| match &event.change {
                    Change::Mark(marked) if marked == name => Some(event.time),
                    _ => None,
                })
                .ok_or_else(|| {
                    anyhow!(
                        "no mark named {name:?} in {} {}; name a point with `agent-terminal mark {name}`",
                        self.session,
                        self.tab
                    )
                }),
            ClipPoint::Activity(entry) => {
                let (began, took) = call(self.folder, self.session, *entry)?;
                self.seconds(match edge {
                    Edge::Start => began,
                    Edge::End => began + took + 2 * TRUNCATED,
                })
            }
        }
    }

    fn command(&self, id: u32) -> Result<CommandRecord> {
        let commands = self.terminal.commands().ok_or_else(|| {
            anyhow!(
                "{} {} runs {} without shell integration, so it keeps no command history; clip --command works in a tab opened with `agent-terminal shell`",
                self.session,
                self.tab,
                self.terminal.program
            )
        })?;
        commands.into_iter().find(|command| command.id == id).ok_or_else(|| {
            anyhow!("no command {id} in {} {}; list them with `agent-terminal history`", self.session, self.tab)
        })
    }

    fn seconds(&self, at: SystemTime) -> Result<f64> {
        let since_epoch = at.duration_since(UNIX_EPOCH).context("a time before 1970 is not in any recording")?;
        Ok(since_epoch.as_secs_f64() - self.cast.timestamp as f64)
    }
}

#[derive(Clone, Copy)]
enum Edge {
    Start,
    End,
}

#[derive(Deserialize)]
struct Call {
    time: String,
    duration_ms: u64,
}

fn call(folder: &SessionFolder, session: &str, entry: usize) -> Result<(SystemTime, Duration)> {
    let (calls, _) = folder.activity_since(0)?;
    let logged = calls.get(entry - 1).ok_or_else(|| {
        let entries = if calls.len() == 1 { "1 entry".to_string() } else { format!("{} entries", calls.len()) };
        anyhow!("@activity:{entry} is not in the activity log of session {session}, which has {entries}")
    })?;
    let call = Call::deserialize(logged).with_context(|| format!("read entry {entry} of the activity log"))?;
    let began = humantime::parse_rfc3339(&call.time)
        .with_context(|| format!("read the time of entry {entry} of the activity log"))?;
    Ok((began, Duration::from_millis(call.duration_ms)))
}

struct Cast {
    size: Size,
    timestamp: u64,
    events: Vec<Event>,
}

impl Cast {
    fn read(path: &std::path::Path, until: f64) -> Result<Self> {
        let mut reader = Reader::open(path)?;
        let mut events = Vec::new();
        while let Some(event) = reader.next()? {
            if event.time > until {
                break;
            }
            events.push(event);
        }
        let header = reader.header();
        Ok(Self { size: header.size(), timestamp: header.timestamp, events })
    }
}

fn film(cast: &Cast, from: f64, to: f64, duration_ms: u64, mut movie: impl Movie) -> Result<Filmed> {
    let mut screen = Screen::new(cast.size);
    let mut events = cast.events.iter().peekable();
    let mut play = |screen: &mut Screen, until: f64| {
        while let Some(event) = events.next_if(|event| event.time <= until) {
            event.change.apply(screen);
        }
    };
    play(&mut screen, from);
    let canvas = canvas(screen.size(), cast, from, to);
    let slots = (duration_ms / SLOT_MS).max(1);
    let end_of = |slot: u64| if slot == slots { to } else { from + (slot * SLOT_MS) as f64 / 1000.0 };
    play(&mut screen, end_of(1));
    let mut shown = Shown { look: look(&screen), image: picture(&screen, canvas), start_ms: 0 };
    let mut frames = 0;
    for slot in 2..=slots {
        play(&mut screen, end_of(slot));
        let look = look(&screen);
        if look == shown.look {
            continue;
        }
        let image = picture(&screen, canvas);
        if image.rgba() == shown.image.rgba() {
            shown.look = look;
            continue;
        }
        let start_ms = (slot - 1) * SLOT_MS;
        movie.add(&shown.image, shown.start_ms, start_ms)?;
        frames += 1;
        shown = Shown { look, image, start_ms };
    }
    movie.add(&shown.image, shown.start_ms, duration_ms)?;
    frames += 1;
    let (width, height) = movie.size();
    Ok(Filmed { bytes: movie.finish()?, frames, width, height })
}

fn look(screen: &Screen) -> (u64, Size) {
    (screen.fingerprint(), screen.size())
}

fn canvas(at_start: Size, cast: &Cast, from: f64, to: f64) -> Size {
    cast.events
        .iter()
        .filter(|event| event.time > from && event.time <= to)
        .filter_map(|event| match event.change {
            Change::Resize(size) => Some(size),
            Change::Output(_) | Change::Mark(_) | Change::Input => None,
        })
        .fold(at_start, |canvas, size| Size { cols: canvas.cols.max(size.cols), rows: canvas.rows.max(size.rows) })
}

fn picture(screen: &Screen, canvas: Size) -> Image {
    let grid = screen.grid();
    let grid = if screen.size() == canvas { grid } else { fit(grid, canvas) };
    render::image(&grid, RowRange { first: 1, last: canvas.rows }, &Look::default(), &[])
}

fn fit(grid: Grid, canvas: Size) -> Grid {
    let mut rows = Screen::new(canvas).grid().rows;
    for (row, cells) in rows.iter_mut().zip(grid.rows) {
        for (cell, glyph) in row.iter_mut().zip(cells) {
            *cell = glyph;
        }
    }
    Grid { rows, cursor: grid.cursor }
}
