use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::{Cell, RowRange};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "until", rename_all = "kebab-case")]
pub(crate) enum Wait {
    Stable { quiet_ms: u64 },
    Idle { quiet_ms: u64 },
    Text(TextMatch),
    Gone(TextMatch),
    Exit { code: Option<i32> },
    Cursor { row: u16, col: u16 },
    Frame { frame: u64 },
    Elapsed { ms: u64 },
    Command,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct TextMatch {
    pub text: String,
    pub regex: bool,
    pub rows: Option<RowRange>,
}

impl TextMatch {
    pub(crate) fn plain(text: String) -> Self {
        Self { text, regex: false, rows: None }
    }
}

impl fmt::Display for TextMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.regex { write!(f, "/{}/", self.text) } else { write!(f, "\"{}\"", self.text) }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "check", rename_all = "kebab-case")]
pub(crate) enum Expect {
    Text(TextMatch),
    Gone(TextMatch),
    ExitCode { code: i32 },
    Cell { row: u16, col: u16, style: Vec<Style> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Style {
    Bold,
    Inverse,
    Underline,
    Fg(String),
    Bg(String),
}

impl Style {
    pub(crate) fn holds(&self, cell: &Cell) -> bool {
        match self {
            Style::Bold => cell.bold,
            Style::Inverse => cell.inverse,
            Style::Underline => cell.underline,
            Style::Fg(color) => cell.fg == *color,
            Style::Bg(color) => cell.bg == *color,
        }
    }
}

impl FromStr for Style {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let lower = text.to_ascii_lowercase();
        let style = match lower.split_once('=') {
            None => flag(&lower),
            Some((key, color)) => colored(key, color),
        };
        style.ok_or_else(|| format!("unknown style {text}; use bold, inverse, underline, fg=<color> or bg=<color>"))
    }
}

fn flag(name: &str) -> Option<Style> {
    match name {
        "bold" => Some(Style::Bold),
        "inverse" => Some(Style::Inverse),
        "underline" => Some(Style::Underline),
        _ => None,
    }
}

fn colored(key: &str, color: &str) -> Option<Style> {
    let color = Some(color).filter(|color| !color.is_empty())?.to_string();
    match key {
        "fg" => Some(Style::Fg(color)),
        "bg" => Some(Style::Bg(color)),
        _ => None,
    }
}

impl fmt::Display for Style {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Style::Bold => f.write_str("bold"),
            Style::Inverse => f.write_str("inverse"),
            Style::Underline => f.write_str("underline"),
            Style::Fg(color) => write!(f, "fg={color}"),
            Style::Bg(color) => write!(f, "bg={color}"),
        }
    }
}
