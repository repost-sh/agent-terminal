use std::path::PathBuf;
use std::str::Chars;
use std::time::Duration;

use crate::protocol::{RowRange, Scrollback, Size};

#[derive(Clone)]
pub(super) struct RawBytes(pub(super) Vec<u8>);

#[derive(Clone)]
pub(super) enum ScreenshotPath {
    Default,
    At(PathBuf),
}

impl From<&str> for ScreenshotPath {
    fn from(value: &str) -> Self {
        if value.is_empty() { ScreenshotPath::Default } else { ScreenshotPath::At(PathBuf::from(value)) }
    }
}

impl ScreenshotPath {
    pub(super) fn chosen(self) -> Option<PathBuf> {
        match self {
            ScreenshotPath::Default => None,
            ScreenshotPath::At(path) => Some(path),
        }
    }
}

pub(super) fn parse_duration(value: &str) -> Result<Duration, String> {
    let refused = || format!("invalid time: {value} (expected a duration like 30s, 10m, 1h or raw ms (0 = never))");
    let unit_at = value.find(|c: char| !c.is_ascii_digit()).unwrap_or(value.len());
    let (number, unit) = value.split_at(unit_at);
    let number: u64 = number.parse().map_err(|e| format!("{}: {e}", refused()))?;
    let millis_per_unit = match unit {
        "" | "ms" => 1,
        "s" => 1_000,
        "m" => 60_000,
        "h" => 3_600_000,
        _ => return Err(refused()),
    };
    number.checked_mul(millis_per_unit).map(Duration::from_millis).ok_or_else(refused)
}

pub(super) fn parse_size(value: &str) -> Result<Size, String> {
    let (cols, rows) = value.split_once('x').ok_or("expected COLSxROWS, e.g. 120x40")?;
    let cols: u16 = cols.parse().map_err(|e| format!("invalid column count: {cols} ({e})"))?;
    let rows: u16 = rows.parse().map_err(|e| format!("invalid row count: {rows} ({e})"))?;
    Size::new(cols, rows)
}

pub(super) fn parse_rows(value: &str) -> Result<RowRange, String> {
    let (first, last) = value.split_once(':').ok_or("expected A:B, e.g. 1:10")?;
    let first: u16 = first.parse().map_err(|e| format!("invalid row: {first} ({e})"))?;
    let last: u16 = last.parse().map_err(|e| format!("invalid row: {last} ({e})"))?;
    if first == 0 || first > last {
        return Err("rows count from 1, and A must not be after B".into());
    }
    Ok(RowRange { first, last })
}

pub(super) fn parse_scrollback(value: &str) -> Result<Scrollback, String> {
    if value.is_empty() {
        return Ok(Scrollback::All);
    }
    value.parse().map(Scrollback::Last).map_err(|e| format!("invalid line count: {value} ({e})"))
}

pub(super) fn parse_threshold(value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|share| (0.0..=1.0).contains(share))
        .ok_or_else(|| format!("invalid threshold: {value} (expected a share of pixels from 0 to 1, e.g. 0.01)"))
}

pub(super) fn parse_position(value: &str) -> Result<(u16, u16), String> {
    let (row, col) = value.split_once(',').ok_or("expected ROW,COL, e.g. 4,3")?;
    let row: u16 = row.parse().map_err(|e| format!("invalid row: {row} ({e})"))?;
    let col: u16 = col.parse().map_err(|e| format!("invalid column: {col} ({e})"))?;
    if row == 0 || col == 0 {
        return Err("rows and columns count from 1".into());
    }
    Ok((row, col))
}

pub(super) fn parse_origin(value: &str) -> Result<String, String> {
    let refused = || format!("{value} is not an origin; an origin is http://host:port, like http://localhost:5173");
    let rest = value.strip_prefix("http://").or_else(|| value.strip_prefix("https://")).ok_or_else(refused)?;
    let (host, port) = rest.rsplit_once(':').ok_or_else(refused)?;
    if host.is_empty() || host.contains('/') || port.parse::<u16>().is_err() {
        return Err(refused());
    }
    Ok(value.to_string())
}

pub(super) fn parse_env(value: &str) -> Result<(String, String), String> {
    let (key, value) = value.split_once('=').ok_or("expected K=V")?;
    if key.is_empty() {
        return Err("expected K=V".into());
    }
    Ok((key.to_string(), value.to_string()))
}

pub(super) fn parse_bytes(value: &str) -> Result<RawBytes, String> {
    let mut bytes = Vec::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            bytes.push(escaped(&mut chars)?);
        } else {
            bytes.extend_from_slice(c.encode_utf8(&mut [0u8; 4]).as_bytes());
        }
    }
    Ok(RawBytes(bytes))
}

fn escaped(chars: &mut Chars<'_>) -> Result<u8, String> {
    match chars.next() {
        Some('e') => Ok(0x1b),
        Some('n') => Ok(b'\n'),
        Some('r') => Ok(b'\r'),
        Some('t') => Ok(b'\t'),
        Some('0') => Ok(0),
        Some('\\') => Ok(b'\\'),
        Some('x') => hex_byte(chars),
        Some(other) => Err(format!(r"unknown escape \{other}; use \e, \xHH, \n, \r, \t, \0 or \\")),
        None => Err(r"a trailing \ escapes nothing; write \\ for a backslash".into()),
    }
}

fn hex_byte(chars: &mut Chars<'_>) -> Result<u8, String> {
    let hex: String = chars.by_ref().take(2).collect();
    Some(&hex)
        .filter(|hex| hex.len() == 2 && hex.chars().all(|c| c.is_ascii_hexdigit()))
        .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        .ok_or_else(|| format!(r"\x needs two hex digits, got \x{hex}"))
}
