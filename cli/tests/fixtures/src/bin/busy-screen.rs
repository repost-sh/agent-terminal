use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

const WORDS: [&str; 12] = [
    "agent", "terminal", "snapshot", "frame", "screen", "session", "daemon", "cursor", "scroll", "render", "glyph",
    "tab",
];
const STYLES: [&str; 8] = ["31", "32", "33", "34", "35", "36", "1;91", "3;94"];

fn main() -> std::io::Result<()> {
    let mut scrollback = 0usize;
    let mut stamps: Option<File> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--scrollback" => scrollback = args.next().and_then(|v| v.parse().ok()).expect("--scrollback LINES"),
            "--stamps" => {
                let path = args.next().expect("--stamps FILE");
                stamps = Some(File::options().create(true).append(true).open(path)?);
            }
            other => panic!("unknown argument: {other}"),
        }
    }

    crossterm::terminal::enable_raw_mode()?;
    let (cols, rows) = crossterm::terminal::size()?;
    let mut out = std::io::stdout().lock();
    let mut word = 0;
    for _ in 0..scrollback + usize::from(rows) - 1 {
        out.write_all(line(usize::from(cols) - 1, &mut word).as_bytes())?;
        out.write_all(b"\r\n")?;
    }
    out.write_all(b"ready")?;
    out.flush()?;

    let mut seen = 0u64;
    let mut input = [0u8; 64];
    loop {
        let read = match std::io::stdin().read(&mut input) {
            Ok(0) => return Ok(()),
            Ok(read) => read,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        for _ in 0..read {
            seen += 1;
            if let Some(stamps) = &mut stamps {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("the clock is after 1970");
                writeln!(stamps, "{seen} {}", now.as_nanos())?;
            }
            write!(out, "\x1b[1;1Hseen {seen}.")?;
            out.flush()?;
        }
    }
}

fn line(width: usize, word: &mut usize) -> String {
    let mut text = String::new();
    let mut used = 0;
    while used < width {
        let next = WORDS[*word % WORDS.len()];
        let style = STYLES[*word % STYLES.len()];
        *word += 1;
        let shown = &next[..next.len().min(width - used)];
        text.push_str(&format!("\x1b[{style}m{shown}\x1b[0m"));
        used += shown.len();
        if used < width {
            text.push(' ');
            used += 1;
        }
    }
    text
}
