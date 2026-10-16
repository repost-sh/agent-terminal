use std::io::{Read, Write};
use std::process::ExitCode;
use std::time::Duration;

use at_fixtures::{Failure, until_hang_up};
use crossterm::terminal;

struct Options {
    flash: bool,
    every: Duration,
}

impl Options {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut options = Options { flash: false, every: Duration::from_millis(10) };
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--flash" => options.flash = true,
                "--repaint" => options.flash = false,
                "--every" => {
                    let every = args.next().and_then(|value| value.parse().ok()).ok_or("--every MS")?;
                    options.every = Duration::from_millis(every);
                }
                other => return Err(format!("unknown argument: {other}")),
            }
        }
        Ok(options)
    }
}

fn main() -> Result<ExitCode, String> {
    let options = Options::parse(std::env::args().skip(1))?;
    Ok(until_hang_up(move || tick(&options), || Ok(())))
}

fn tick(options: &Options) -> Result<(), Failure> {
    terminal::enable_raw_mode()?;
    show("ready\r\n\x1b[6n")?;
    seen()?;
    (1u64..).try_for_each(|tick| {
        if options.flash {
            show(&format!("tick {tick}\x1b[6n"))?;
            seen()?;
            show("\r\x1b[K\x1b[6n")?;
            seen()?;
        } else {
            show("\x1b[1;1Hready\x1b[2;1H")?;
        }
        std::thread::sleep(options.every);
        Ok(())
    })
}

fn show(text: &str) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    out.write_all(text.as_bytes())?;
    out.flush()
}

fn seen() -> std::io::Result<()> {
    let mut byte = [0u8; 1];
    let mut stdin = std::io::stdin().lock();
    loop {
        stdin.read_exact(&mut byte)?;
        if byte == *b"R" {
            return Ok(());
        }
    }
}
