use std::fmt;
use std::io::Write;
use std::process::ExitCode;

use unicode_width::{UNICODE_VERSION, UnicodeWidthChar};

fn width(codepoint: u32) -> usize {
    char::from_u32(codepoint).map_or(1, |c| c.width().unwrap_or(0))
}

fn ranges(of: usize) -> Vec<(u32, u32)> {
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for codepoint in (0..=u32::from(char::MAX)).filter(|&codepoint| width(codepoint) == of) {
        match ranges.last_mut() {
            Some((_, end)) if *end + 1 == codepoint => *end = codepoint,
            _ => ranges.push((codepoint, codepoint)),
        }
    }
    ranges
}

fn table(name: &str, ranges: &[(u32, u32)]) -> String {
    let rows = fmt::from_fn(|f| ranges.iter().try_for_each(|(start, end)| writeln!(f, "  [{start}, {end}],")));
    format!("export const {name}: readonly (readonly [number, number])[] = [\n{rows}];\n")
}

fn main() -> std::io::Result<ExitCode> {
    let text = match std::env::args().nth(1).as_deref() {
        Some("version") => {
            let (major, minor, patch) = UNICODE_VERSION;
            format!("export const UNICODE = \"{major}.{minor}.{patch}\";\n")
        }
        Some("zero") => table("ZERO", &ranges(0)),
        Some("wide") => table("WIDE", &ranges(2)),
        _ => {
            let mut err = std::io::stderr().lock();
            writeln!(err, "usage: width-table version|zero|wide")?;
            return Ok(ExitCode::FAILURE);
        }
    };
    let mut out = std::io::stdout().lock();
    out.write_all(text.as_bytes())?;
    out.flush()?;
    Ok(ExitCode::SUCCESS)
}
