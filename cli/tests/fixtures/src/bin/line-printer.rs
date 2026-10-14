use std::io::{Read, Write};
use std::time::Duration;

fn main() {
    let mut lines = 3u32;
    let mut delay_ms = 0u64;
    let mut exit_code = 0i32;
    let mut hold = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--lines" => lines = args.next().and_then(|v| v.parse().ok()).expect("--lines N"),
            "--delay-ms" => delay_ms = args.next().and_then(|v| v.parse().ok()).expect("--delay-ms MS"),
            "--exit" => exit_code = args.next().and_then(|v| v.parse().ok()).expect("--exit CODE"),
            "--hold" => hold = true,
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
    if hold {
        let _ = std::io::stdin().read(&mut [0u8; 1]);
    }
    std::process::exit(exit_code);
}
