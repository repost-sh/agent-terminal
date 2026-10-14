use std::io::{Read, Write};
use std::time::Duration;

use crossterm::terminal;

fn main() -> std::io::Result<()> {
    let mut flash = false;
    let mut every = Duration::from_millis(10);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--flash" => flash = true,
            "--repaint" => flash = false,
            "--every" => every = Duration::from_millis(args.next().and_then(|v| v.parse().ok()).expect("--every MS")),
            other => panic!("unknown argument: {other}"),
        }
    }

    terminal::enable_raw_mode()?;
    show("ready\r\n\x1b[6n")?;
    seen()?;
    for tick in 1u64.. {
        if flash {
            show(&format!("tick {tick}\x1b[6n"))?;
            seen()?;
            show("\r\x1b[K\x1b[6n")?;
            seen()?;
        } else {
            show("\x1b[1;1Hready\x1b[2;1H")?;
        }
        std::thread::sleep(every);
    }
    Ok(())
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
        if byte[0] == b'R' {
            return Ok(());
        }
    }
}
