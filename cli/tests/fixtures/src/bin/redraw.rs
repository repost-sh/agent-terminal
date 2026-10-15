use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const LOGO: [&str; 7] = [
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

#[derive(Clone, Copy)]
enum Mode {
    Full,
    Partial,
}

struct Stats {
    file: Option<File>,
    frames: u64,
    bytes: u64,
    written: Instant,
}

impl Stats {
    fn count(&mut self, bytes: usize) -> std::io::Result<()> {
        self.frames += 1;
        self.bytes += bytes as u64;
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

fn main() -> std::io::Result<()> {
    let mut hz = 60u32;
    let mut mode = Mode::Partial;
    let mut stats: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--hz" => hz = args.next().and_then(|v| v.parse().ok()).filter(|hz| *hz > 0).expect("--hz N"),
            "--full" => mode = Mode::Full,
            "--partial" => mode = Mode::Partial,
            "--stats" => stats = Some(args.next().expect("--stats FILE").into()),
            other => panic!("unknown argument: {other}"),
        }
    }

    crossterm::terminal::enable_raw_mode()?;
    let (cols, rows) = crossterm::terminal::size()?;
    show(b"\x1b[?1049h\x1b[?25l\x1b[2J\x1b[1;1Hready")?;
    std::thread::spawn(echo);

    let mut stats = Stats { file: stats.map(File::create).transpose()?, frames: 0, bytes: 0, written: Instant::now() };
    let every = Duration::from_secs_f64(1.0 / f64::from(hz));
    let started = Instant::now();
    for tick in 1u64.. {
        let frame = match mode {
            Mode::Full => full(tick, cols, rows),
            Mode::Partial => partial(tick, cols, rows),
        };
        show(frame.as_bytes())?;
        stats.count(frame.len())?;
        let next = started + every.mul_f64(tick as f64);
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
    let mut seen = 0u64;
    let mut input = [0u8; 64];
    loop {
        let read = std::io::stdin().read(&mut input)?;
        if read == 0 {
            return Ok(());
        }
        seen += read as u64;
        show(format!("\x1b[1;1Hseen {seen}.\x1b[K").as_bytes())?;
    }
}

fn partial(tick: u64, cols: u16, rows: u16) -> String {
    let shade = 96 + (tick % 64) as u8;
    let top = rows.saturating_sub(LOGO.len() as u16) / 2;
    let left = cols.saturating_sub(14) / 2;
    let mut frame = String::from("\x1b[?2026h");
    for (index, line) in LOGO.iter().enumerate() {
        let colour = shade.saturating_add(index as u8);
        frame.push_str(&format!(
            "\x1b[{};{}H\x1b[38;2;{colour};{colour};{colour};48;2;20;20;20m{line}",
            top + index as u16,
            left
        ));
    }
    let spinner = SPINNER[(tick % SPINNER.len() as u64) as usize];
    frame.push_str(&format!(
        "\x1b[0m\x1b[{};{}H\x1b[38;2;120;180;255m{spinner}\x1b[0m forwarding {} events, tick {tick}\x1b[K",
        top + LOGO.len() as u16 + 2,
        left.saturating_sub(8),
        tick / 7
    ));
    frame.push_str("\x1b[?25l\x1b[?2026l");
    frame
}

fn full(tick: u64, cols: u16, rows: u16) -> String {
    let mut frame = String::from("\x1b[?2026h\x1b[1;1H");
    for row in 0..rows {
        let mut used = 0usize;
        let mut word = tick as usize + row as usize;
        let mut line = String::new();
        while used < cols as usize {
            let next = WORDS[word % WORDS.len()];
            let shown = &next[..next.len().min(cols as usize - used)];
            let colour = 31 + (word % 6);
            line.push_str(&format!("\x1b[{colour}m{shown}"));
            used += shown.len();
            if used < cols as usize {
                line.push(' ');
                used += 1;
            }
            word += 1;
        }
        frame.push_str(&format!("\x1b[{};1H{line}\x1b[0m", row + 1));
    }
    frame.push_str("\x1b[?2026l");
    frame
}
