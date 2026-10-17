use std::error::Error;
use std::fmt::{self, Write as _};
use std::io::{ErrorKind, Read, Write};
use std::process::ExitCode;

use at_fixtures::until_hang_up;
use crossterm::terminal;
use signal_hook::consts::signal::{SIGCONT, SIGINT, SIGTERM, SIGTSTP, SIGWINCH};
use signal_hook::iterator::Signals;

struct Options {
    modes: String,
    reads: bool,
}

impl Options {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut options = Options { modes: String::new(), reads: false };
        while let Some(arg) = args.next() {
            if arg == "--reads" {
                options.reads = true;
            } else {
                options.modes.push_str(&mode(&arg, &mut args)?);
            }
        }
        Ok(options)
    }
}

fn mode(arg: &str, args: &mut impl Iterator<Item = String>) -> Result<String, String> {
    let mode = match arg {
        "--app-cursor" => "\x1b[?1h",
        "--bracketed-paste" => "\x1b[?2004h",
        "--focus-events" => "\x1b[?1004h",
        "--mouse" => mouse(args.next().as_deref())?,
        "--mouse-encoding" => mouse_encoding(args.next().as_deref())?,
        "--kitty" => {
            let flags: u8 = args.next().and_then(|value| value.parse().ok()).ok_or("--kitty FLAGS")?;
            return Ok(format!("\x1b[>{flags}u"));
        }
        other => return Err(format!("unknown argument: {other}")),
    };
    Ok(mode.to_string())
}

fn mouse(value: Option<&str>) -> Result<&'static str, String> {
    match value {
        Some("click") => Ok("\x1b[?1000h"),
        Some("drag") => Ok("\x1b[?1002h"),
        Some("motion") => Ok("\x1b[?1003h"),
        other => Err(format!("--mouse click|drag|motion, not {other:?}")),
    }
}

fn mouse_encoding(value: Option<&str>) -> Result<&'static str, String> {
    match value {
        Some("sgr") => Ok("\x1b[?1006h"),
        Some("utf8") => Ok("\x1b[?1005h"),
        other => Err(format!("--mouse-encoding sgr|utf8, not {other:?}")),
    }
}

fn main() -> Result<ExitCode, Box<dyn Error>> {
    let options = Options::parse(std::env::args().skip(1))?;
    let mut signals = Signals::new([SIGINT, SIGTERM, SIGTSTP, SIGCONT, SIGWINCH])?;
    std::thread::spawn(move || signals.forever().try_for_each(|signal| show(&format!("[{}]", name(signal)))));
    Ok(until_hang_up(move || start(&options), || show("[HUP]")))
}

fn start(options: &Options) -> std::io::Result<()> {
    terminal::enable_raw_mode()?;
    show(&format!("{}ready\r\n", options.modes))?;
    echo(options.reads)
}

fn name(signal: i32) -> &'static str {
    match signal {
        SIGINT => "INT",
        SIGTERM => "TERM",
        SIGTSTP => "TSTP",
        SIGCONT => "CONT",
        SIGWINCH => "WINCH",
        _ => "?",
    }
}

fn echo(reads: bool) -> std::io::Result<()> {
    let mut stdin = std::io::stdin();
    let mut buf = [0u8; 1024];
    loop {
        match stdin.read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => {
                let read = buf.get(..n).ok_or_else(|| std::io::Error::other("read past the buffer"))?;
                show(&format!("{}{}", escape(read), if reads { "|" } else { "" }))?;
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
}

fn show(text: &str) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    out.write_all(text.as_bytes())?;
    out.flush()
}

fn escape(bytes: &[u8]) -> impl fmt::Display {
    fmt::from_fn(move |f| {
        bytes.iter().try_for_each(|&byte| match byte {
            0x1b => f.write_str("\\e"),
            b'\r' => f.write_str("\\r"),
            b'\n' => f.write_str("\\n"),
            b'\t' => f.write_str("\\t"),
            b'\\' => f.write_str("\\\\"),
            0x20..=0x7e => f.write_char(char::from(byte)),
            _ => write!(f, "\\x{byte:02x}"),
        })
    })
}
