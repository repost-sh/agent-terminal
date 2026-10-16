use std::fmt;

use serde::{Deserialize, Serialize};

use super::Size;

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Get {
    Text,
    Cursor,
    Size,
    Frame,
    Program,
    ExitCode,
    Pid,
    Title,
    Cwd,
    Modes,
    Cell { row: u16, col: u16 },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Fact {
    Text(String),
    Cursor(Cursor),
    Size(Size),
    Frame(u64),
    Program(String),
    ExitCode(Exit),
    Pid(Option<u32>),
    Title(Option<String>),
    Cwd(String),
    Modes(Modes),
    Cell(Cell),
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Cell {
    pub row: u16,
    pub col: u16,
    pub char: String,
    pub fg: String,
    pub bg: String,
    pub bold: bool,
    pub inverse: bool,
    pub underline: bool,
}

impl fmt::Display for Cell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} fg={} bg={}", self.char, self.fg, self.bg)?;
        for (on, name) in [(self.bold, "bold"), (self.inverse, "inverse"), (self.underline, "underline")] {
            if on {
                write!(f, " {name}")?;
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct Modes {
    pub alt_screen: Switch,
    pub mouse: Option<Mouse>,
    pub bracketed_paste: Switch,
    pub app_cursor: Switch,
    pub focus_events: Switch,
    pub keyboard: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(from = "bool", into = "bool")]
pub(crate) enum Switch {
    On,
    Off,
}

impl From<bool> for Switch {
    fn from(on: bool) -> Self {
        if on { Switch::On } else { Switch::Off }
    }
}

impl From<Switch> for bool {
    fn from(switch: Switch) -> Self {
        switch == Switch::On
    }
}

impl fmt::Display for Switch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Switch::On => "on",
            Switch::Off => "off",
        })
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct Mouse {
    pub tracking: String,
    pub encoding: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Exit {
    pub exited: bool,
    pub code: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Cursor {
    pub row: u16,
    pub col: u16,
    pub visible: bool,
}
