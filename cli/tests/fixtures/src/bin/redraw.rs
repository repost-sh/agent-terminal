use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::iter::{Cycle, Zip};
use std::ops::Range;
use std::path::PathBuf;
use std::slice::Iter;
use std::time::{Duration, Instant};

const LOGO_HEIGHT: u16 = 7;
const LOGO: [&str; LOGO_HEIGHT as usize] = [
    "⠀⠐⢶⣶⣶⣶⣶⣶⣶⣦⣄⠀⠀⠀",
    "⠀⣴⣶⣿⠛⠛⠛⠛⠛⢿⣿⣷⠀⠀",
    "⠀⣿⣿⣿⠀⠀⠀⠀⠀⣸⣿⣿⠇⠀",
    "⠀⣿⣿⣿⣶⣶⣶⣶⣾⣿⣿⠏⠀⠀",
    "⠀⣿⣿⣿⠛⠛⠛⢻⣿⣿⡄⠀⠀⠀",
    "⠀⣿⣿⣿⠀⠀⠀⠀⠻⣿⣿⡄⠀⠀",
    "⠀⠿⠿⠿⠀⠀⠀⠀⠀⠹⠿⠿⠆⠀",
];
const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const WORDS: [&str; 8] = ["forward", "webhook", "bucket", "delivery", "attempt", "event", "endpoint", "replay"];

type Words = Zip<Cycle<Iter<'static, &'static str>>, Cycle<Range<u8>>>;

#[derive(Clone, Copy)]
enum Mode {
    Full,
    Partial,
}

struct Options {
    hz: u32,
    mode: Mode,
    stats: Option<PathBuf>,
}

impl Options {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut options = Options { hz: 60, mode: Mode::Partial, stats: None };
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--hz" => {
                    options.hz =
                        args.next().and_then(|value| value.parse().ok()).filter(|hz| *hz > 0).ok_or("--hz N")?;
                }
                "--full" => options.mode = Mode::Full,
                "--partial" => options.mode = Mode::Partial,
                "--stats" => options.stats = Some(args.next().ok_or("--stats FILE")?.into()),
                other => return Err(format!("unknown argument: {other}")),
            }
        }
        Ok(options)
    }
}

struct Stats {
    file: Option<File>,
    frames: u64,
    bytes: usize,
    written: Instant,
}

impl Stats {
    fn count(&mut self, bytes: usize) -> std::io::Result<()> {
        self.frames += 1;
        self.bytes += bytes;
        if self.written.elapsed() < Duration::from_millis(250) {
            return Ok(());
        }
        self.written = Instant::now();
        if let Some(file) = &mut self.file {
            file.set_len(0)?;
            file.seek(SeekFrom::Start(0))?;
            writeln!(file, "{} {}", self.frames, self.bytes)?;
        }
        Ok(())
    }
}

#[derive(Clone)]
struct Tick {
    count: u64,
    shade: u8,
    spinner: char,
    words: Words,
}

fn ticks() -> impl Iterator<Item = Tick> {
    let words = WORDS.iter().cycle().zip((31u8..37).cycle());
    (1u64..).zip((96u8..160).cycle().skip(1)).zip(SPINNER.iter().cycle().skip(1)).scan(
        words,
        |words, ((count, shade), &spinner)| {
            words.next();
            Some(Tick { count, shade, spinner, words: words.clone() })
        },
    )
}

fn main() -> Result<(), Box<dyn Error>> {
    let options = Options::parse(std::env::args().skip(1))?;
    crossterm::terminal::enable_raw_mode()?;
    let (cols, rows) = crossterm::terminal::size()?;
    show(b"\x1b[?1049h\x1b[?25l\x1b[2J\x1b[1;1Hready")?;
    std::thread::spawn(echo);

    let file = options.stats.map(File::create).transpose()?;
    let mut stats = Stats { file, frames: 0, bytes: 0, written: Instant::now() };
    let every = Duration::from_secs_f64(1.0 / f64::from(options.hz));
    let mut next = Instant::now();
    for tick in ticks() {
        let frame = match options.mode {
            Mode::Full => full(tick, cols, rows).to_string(),
            Mode::Partial => partial(&tick, cols, rows).to_string(),
        };
        show(frame.as_bytes())?;
        stats.count(frame.len())?;
        next += every;
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    Ok(())
}

fn show(bytes: &[u8]) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    out.write_all(bytes)?;
    out.flush()
}

fn echo() -> std::io::Result<()> {
    let mut seen = 0usize;
    let mut input = [0u8; 64];
    loop {
        let read = std::io::stdin().read(&mut input)?;
        if read == 0 {
            return Ok(());
        }
        seen += read;
        show(format!("\x1b[1;1Hseen {seen}.\x1b[K").as_bytes())?;
    }
}

fn partial(tick: &Tick, cols: u16, rows: u16) -> impl fmt::Display {
    let top = rows.saturating_sub(LOGO_HEIGHT) / 2;
    let left = cols.saturating_sub(14) / 2;
    fmt::from_fn(move |f| {
        f.write_str("\x1b[?2026h")?;
        for ((line, row), offset) in LOGO.iter().zip(top..).zip(0u8..) {
            let colour = tick.shade.saturating_add(offset);
            write!(f, "\x1b[{row};{left}H\x1b[38;2;{colour};{colour};{colour};48;2;20;20;20m{line}")?;
        }
        write!(
            f,
            "\x1b[0m\x1b[{};{}H\x1b[38;2;120;180;255m{}\x1b[0m forwarding {} events, tick {}\x1b[K",
            top + LOGO_HEIGHT + 2,
            left.saturating_sub(8),
            tick.spinner,
            tick.count / 7,
            tick.count
        )?;
        f.write_str("\x1b[?25l\x1b[?2026l")
    })
}

fn full(tick: Tick, cols: u16, rows: u16) -> impl fmt::Display {
    fmt::from_fn(move |f| {
        f.write_str("\x1b[?2026h\x1b[1;1H")?;
        let mut words = tick.words.clone();
        for row in 1..=rows {
            write!(f, "\x1b[{row};1H{}\x1b[0m", line(words.clone(), usize::from(cols)))?;
            words.next();
        }
        f.write_str("\x1b[?2026l")
    })
}

fn line(words: Words, width: usize) -> impl fmt::Display {
    fmt::from_fn(move |f| {
        let mut used = 0;
        for (next, colour) in words.clone() {
            if used >= width {
                break;
            }
            let shown = next.len().min(width - used);
            write!(f, "\x1b[{colour}m{next:.shown$}")?;
            used += shown;
            if used < width {
                f.write_str(" ")?;
                used += 1;
            }
        }
        Ok(())
    })
}
