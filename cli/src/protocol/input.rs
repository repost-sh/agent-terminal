use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::{Name, Screenshot, Snapshot};
use crate::key::Key;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum Input {
    Type { text: String, delay_ms: u64 },
    Press { keys: Vec<Key>, repeat: u32 },
    Paste { text: String },
    Send { bytes: Vec<u8> },
    Signal { signal: Signal },
    Click { target: ClickTarget, button: MouseButton, double: bool },
    Mouse { event: MouseEvent },
    Scroll { direction: Direction, count: u32, keys: bool },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(try_from = "String", into = "String")]
pub enum ClickTarget {
    Ref(u32),
    Cell { row: u16, col: u16 },
    Text(String),
}

impl FromStr for ClickTarget {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        if let Some(id) = text.strip_prefix("@t").and_then(|id| id.parse().ok()).filter(|&id: &u32| id > 0) {
            return Ok(ClickTarget::Ref(id));
        }
        let cell = text.split_once(',').and_then(|(row, col)| Some((row.parse().ok()?, col.parse().ok()?)));
        match cell {
            Some((0, _) | (_, 0)) => Err("rows and columns count from 1".into()),
            Some((row, col)) => Ok(ClickTarget::Cell { row, col }),
            None if text.is_empty() => Err("give a ref (@t3), a cell (ROW,COL) or the text to click".into()),
            None => Ok(ClickTarget::Text(text.to_string())),
        }
    }
}

impl fmt::Display for ClickTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClickTarget::Ref(id) => write!(f, "@t{id}"),
            ClickTarget::Cell { row, col } => write!(f, "{row},{col}"),
            ClickTarget::Text(text) => f.write_str(text),
        }
    }
}

impl TryFrom<String> for ClickTarget {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        text.parse()
    }
}

impl From<ClickTarget> for String {
    fn from(target: ClickTarget) -> String {
        target.to_string()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MouseButton {
    Left,
    Middle,
    Right,
}

impl fmt::Display for MouseButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            MouseButton::Left => "left",
            MouseButton::Middle => "middle",
            MouseButton::Right => "right",
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Direction {
    Up,
    Down,
}

impl FromStr for Direction {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        match text.to_ascii_lowercase().as_str() {
            "up" => Ok(Direction::Up),
            "down" => Ok(Direction::Down),
            _ => Err(format!("unknown direction {text}; use up or down")),
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Direction::Up => "up",
            Direction::Down => "down",
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub(crate) enum MouseEvent {
    Move { row: u16, col: u16 },
    Down { row: u16, col: u16, button: MouseButton },
    Up { row: u16, col: u16, button: MouseButton },
    Wheel { row: u16, col: u16, direction: Direction },
}

impl MouseEvent {
    pub(crate) fn at(self) -> (u16, u16) {
        match self {
            MouseEvent::Move { row, col }
            | MouseEvent::Down { row, col, .. }
            | MouseEvent::Up { row, col, .. }
            | MouseEvent::Wheel { row, col, .. } => (row, col),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ScrollBy {
    Wheel,
    Keys,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum Signal {
    Int,
    Term,
    Kill,
    Hup,
    Tstp,
    Cont,
    Winch,
}

impl Signal {
    const ALL: [Signal; 7] =
        [Signal::Int, Signal::Term, Signal::Kill, Signal::Hup, Signal::Tstp, Signal::Cont, Signal::Winch];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Signal::Int => "INT",
            Signal::Term => "TERM",
            Signal::Kill => "KILL",
            Signal::Hup => "HUP",
            Signal::Tstp => "TSTP",
            Signal::Cont => "CONT",
            Signal::Winch => "WINCH",
        }
    }
}

impl FromStr for Signal {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let upper = text.to_ascii_uppercase();
        let name = upper.strip_prefix("SIG").unwrap_or(&upper);
        Signal::ALL
            .into_iter()
            .find(|signal| signal.name() == name)
            .ok_or_else(|| format!("unknown signal {text}; use INT, TERM, KILL, HUP, TSTP, CONT or WINCH"))
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Delivered {
    pub session: Name,
    pub tab: Name,
    #[serde(flatten)]
    pub delivery: Delivery,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<Box<Screenshot>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<Box<Snapshot>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "input", rename_all = "kebab-case")]
pub(crate) enum Delivery {
    Text {
        keys: usize,
        bytes: usize,
    },
    Keys {
        keys: usize,
        bytes: usize,
    },
    Paste {
        bracketed: bool,
        bytes: usize,
    },
    Raw {
        bytes: usize,
    },
    Signal {
        signal: Signal,
        process_group: i32,
    },
    Click {
        target: ClickTarget,
        row: u16,
        col: u16,
        button: MouseButton,
        double: bool,
        bytes: usize,
    },
    Mouse {
        #[serde(flatten)]
        event: MouseEvent,
        bytes: usize,
    },
    Scroll {
        direction: Direction,
        count: u32,
        by: ScrollBy,
        bytes: usize,
    },
}
