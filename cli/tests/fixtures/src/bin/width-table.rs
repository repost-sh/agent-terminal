use unicode_width::{UNICODE_VERSION, UnicodeWidthChar};

fn width(codepoint: u32) -> usize {
    char::from_u32(codepoint).map_or(1, |c| c.width().unwrap_or(0))
}

fn ranges(of: usize) -> Vec<(u32, u32)> {
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for codepoint in (0..=0x10FFFF).filter(|&codepoint| width(codepoint) == of) {
        match ranges.last_mut() {
            Some((_, end)) if *end + 1 == codepoint => *end = codepoint,
            _ => ranges.push((codepoint, codepoint)),
        }
    }
    ranges
}

fn table(name: &str, ranges: &[(u32, u32)]) -> String {
    let rows: String = ranges.iter().map(|(start, end)| format!("  [{start}, {end}],\n")).collect();
    format!("export const {name}: readonly (readonly [number, number])[] = [\n{rows}];\n")
}

fn main() {
    let (major, minor, patch) = UNICODE_VERSION;
    print!(
        "export const UNICODE = \"{major}.{minor}.{patch}\";\n\n{}\n{}",
        table("ZERO", &ranges(0)),
        table("WIDE", &ranges(2))
    );
}
