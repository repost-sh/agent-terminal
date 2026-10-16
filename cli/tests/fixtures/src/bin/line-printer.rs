use std::error::Error;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;
use std::time::Duration;

struct Options {
    lines: u32,
    start_after_ms: u64,
    delay_ms: u64,
    exit_code: u8,
    hold: bool,
    cue: Option<PathBuf>,
}

impl Options {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut options = Options { lines: 3, start_after_ms: 0, delay_ms: 0, exit_code: 0, hold: false, cue: None };
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--lines" => options.lines = number(args.next(), "--lines N")?,
                "--start-after-ms" => options.start_after_ms = number(args.next(), "--start-after-ms MS")?,
                "--delay-ms" => options.delay_ms = number(args.next(), "--delay-ms MS")?,
                "--exit" => options.exit_code = number(args.next(), "--exit CODE")?,
                "--hold" => options.hold = true,
                "--cue" => options.cue = Some(args.next().ok_or("--cue SOCKET")?.into()),
                other => return Err(format!("unknown argument: {other}")),
            }
        }
        Ok(options)
    }
}

fn number<T: FromStr>(value: Option<String>, usage: &str) -> Result<T, String> {
    value.and_then(|value| value.parse().ok()).ok_or_else(|| usage.to_string())
}

fn main() -> Result<ExitCode, Box<dyn Error>> {
    let options = Options::parse(std::env::args().skip(1))?;
    let mut out = std::io::stdout();
    std::thread::sleep(Duration::from_millis(options.start_after_ms));
    for i in 1..=options.lines {
        writeln!(out, "line {i}")?;
        out.flush()?;
        if options.delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(options.delay_ms));
        }
    }
    if let Some(path) = &options.cue {
        print_cues(path)?;
    }
    if options.hold {
        hold()?;
    }
    Ok(ExitCode::from(options.exit_code))
}

fn hold() -> std::io::Result<()> {
    match std::io::stdin().read_exact(&mut [0u8; 1]) {
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => Ok(()),
        held => held,
    }
}

fn print_cues(path: &Path) -> Result<(), Box<dyn Error>> {
    let mut test = UnixStream::connect(path)?;
    let mut out = std::io::stdout();
    loop {
        let mut length = [0u8; 4];
        match test.read_exact(&mut length) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(e.into()),
        }
        let mut cue = vec![0u8; usize::try_from(u32::from_be_bytes(length))?];
        test.read_exact(&mut cue)?;
        out.write_all(&cue)?;
        out.flush()?;
        wait_until_the_terminal_has_read_it()?;
        test.write_all(b"\n")?;
    }
}

fn wait_until_the_terminal_has_read_it() -> std::io::Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    let mut out = std::io::stdout();
    out.write_all(b"\x1b[6n")?;
    out.flush()?;
    let mut byte = [0u8; 1];
    while byte != *b"R" {
        std::io::stdin().read_exact(&mut byte)?;
    }
    crossterm::terminal::disable_raw_mode()
}
