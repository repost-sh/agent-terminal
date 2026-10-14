use std::io::{ErrorKind, Read, Write};

use crossterm::terminal;
use signal_hook::consts::signal::{SIGCONT, SIGHUP, SIGINT, SIGTERM, SIGTSTP, SIGWINCH};
use signal_hook::iterator::Signals;

fn main() -> std::io::Result<()> {
    let mut modes = String::new();
    let mut reads = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--reads" => reads = true,
            "--app-cursor" => modes.push_str("\x1b[?1h"),
            "--bracketed-paste" => modes.push_str("\x1b[?2004h"),
            "--kitty" => {
                let flags: u8 = args.next().and_then(|v| v.parse().ok()).expect("--kitty FLAGS");
                modes.push_str(&format!("\x1b[>{flags}u"));
            }
            other => panic!("unknown argument: {other}"),
        }
    }

    let mut signals = Signals::new([SIGINT, SIGTERM, SIGHUP, SIGTSTP, SIGCONT, SIGWINCH])?;
    std::thread::spawn(move || {
        for signal in signals.forever() {
            let name = match signal {
                SIGINT => "INT",
                SIGTERM => "TERM",
                SIGHUP => "HUP",
                SIGTSTP => "TSTP",
                SIGCONT => "CONT",
                SIGWINCH => "WINCH",
                _ => "?",
            };
            show(&format!("[{name}]"));
        }
    });

    terminal::enable_raw_mode()?;
    show(&format!("{modes}ready\r\n"));
    let mut stdin = std::io::stdin();
    let mut buf = [0u8; 1024];
    loop {
        match stdin.read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => show(&format!("{}{}", escape(&buf[..n]), if reads { "|" } else { "" })),
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
}

fn show(text: &str) {
    let mut out = std::io::stdout().lock();
    out.write_all(text.as_bytes()).and_then(|()| out.flush()).expect("write to the terminal");
}

fn escape(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &byte in bytes {
        match byte {
            0x1b => out.push_str("\\e"),
            b'\r' => out.push_str("\\r"),
            b'\n' => out.push_str("\\n"),
            b'\t' => out.push_str("\\t"),
            b'\\' => out.push_str("\\\\"),
            0x20..=0x7e => out.push(byte as char),
            _ => out.push_str(&format!("\\x{byte:02x}")),
        }
    }
    out
}
