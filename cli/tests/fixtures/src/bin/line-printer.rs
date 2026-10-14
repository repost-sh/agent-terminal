use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn main() {
    let mut lines = 3u32;
    let mut delay_ms = 0u64;
    let mut exit_code = 0i32;
    let mut hold = false;
    let mut cue: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--lines" => lines = args.next().and_then(|v| v.parse().ok()).expect("--lines N"),
            "--delay-ms" => delay_ms = args.next().and_then(|v| v.parse().ok()).expect("--delay-ms MS"),
            "--exit" => exit_code = args.next().and_then(|v| v.parse().ok()).expect("--exit CODE"),
            "--hold" => hold = true,
            "--cue" => cue = Some(args.next().expect("--cue DIR").into()),
            other => panic!("unknown argument: {other}"),
        }
    }
    let mut out = std::io::stdout();
    for i in 1..=lines {
        writeln!(out, "line {i}").unwrap();
        out.flush().unwrap();
        if delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(delay_ms));
        }
    }
    if let Some(dir) = cue {
        print_cues(&dir);
    }
    if hold {
        let _ = std::io::stdin().read(&mut [0u8; 1]);
    }
    std::process::exit(exit_code);
}

fn print_cues(dir: &Path) -> ! {
    let mut out = std::io::stdout();
    loop {
        let mut cue = Vec::new();
        std::fs::File::open(dir.join("cue")).and_then(|mut f| f.read_to_end(&mut cue)).expect("read the cue");
        out.write_all(&cue).and_then(|()| out.flush()).expect("print the cue");
        wait_until_the_terminal_has_read_it();
        std::fs::write(dir.join("ack"), b"printed\n").expect("acknowledge the cue");
    }
}

fn wait_until_the_terminal_has_read_it() {
    crossterm::terminal::enable_raw_mode().expect("raw mode");
    let mut out = std::io::stdout();
    out.write_all(b"\x1b[6n").and_then(|()| out.flush()).expect("ask for the cursor position");
    let mut byte = [0u8; 1];
    while byte[0] != b'R' {
        std::io::stdin().read_exact(&mut byte).expect("read the cursor position report");
    }
    crossterm::terminal::disable_raw_mode().expect("cooked mode");
}
