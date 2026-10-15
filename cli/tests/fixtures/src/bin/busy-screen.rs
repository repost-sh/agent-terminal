use std::error::Error;
use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

const WORDS: [&str; 12] = [
    "agent", "terminal", "snapshot", "frame", "screen", "session", "daemon", "cursor", "scroll", "render", "glyph",
    "tab",
];
const STYLES: [&str; 8] = ["31", "32", "33", "34", "35", "36", "1;91", "3;94"];

struct Options {
    scrollback: usize,
    stamps: Option<File>,
}

impl Options {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, Box<dyn Error>> {
        let mut options = Options { scrollback: 0, stamps: None };
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--scrollback" => {
                    options.scrollback =
                        args.next().and_then(|value| value.parse().ok()).ok_or("--scrollback LINES")?;
                }
                "--stamps" => {
                    let path = args.next().ok_or("--stamps FILE")?;
                    options.stamps = Some(File::options().create(true).append(true).open(path)?);
                }
                other => return Err(format!("unknown argument: {other}").into()),
            }
        }
        Ok(options)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut options = Options::parse(std::env::args().skip(1))?;
    crossterm::terminal::enable_raw_mode()?;
    let (cols, rows) = crossterm::terminal::size()?;
    let mut out = std::io::stdout().lock();
    let mut words = WORDS.iter().cycle().zip(STYLES.iter().cycle());
    for _ in 0..options.scrollback + usize::from(rows) - 1 {
        line(&mut out, usize::from(cols) - 1, &mut words)?;
        out.write_all(b"\r\n")?;
    }
    out.write_all(b"ready")?;
    out.flush()?;
    echo(&mut out, options.stamps.as_mut())
}

fn echo(out: &mut impl Write, mut stamps: Option<&mut File>) -> Result<(), Box<dyn Error>> {
    let mut seen = 0u64;
    let mut input = [0u8; 64];
    loop {
        let read = match std::io::stdin().read(&mut input) {
            Ok(0) => return Ok(()),
            Ok(read) => read,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        };
        for _ in 0..read {
            seen += 1;
            if let Some(stamps) = stamps.as_deref_mut() {
                let now = SystemTime::now().duration_since(UNIX_EPOCH)?;
                writeln!(stamps, "{seen} {}", now.as_nanos())?;
            }
            write!(out, "\x1b[1;1Hseen {seen}.")?;
            out.flush()?;
        }
    }
}

fn line<'a>(
    out: &mut impl Write,
    width: usize,
    words: &mut impl Iterator<Item = (&'a &'a str, &'a &'a str)>,
) -> std::io::Result<()> {
    let mut used = 0;
    while used < width {
        let Some((next, style)) = words.next() else {
            return Ok(());
        };
        let shown = next.len().min(width - used);
        write!(out, "\x1b[{style}m{next:.shown$}\x1b[0m")?;
        used += shown;
        if used < width {
            out.write_all(b" ")?;
            used += 1;
        }
    }
    Ok(())
}
