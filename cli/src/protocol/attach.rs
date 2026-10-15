use serde::{Deserialize, Serialize};

use super::Input;
use crate::key::Key;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum HumanInput {
    Typed(String),
    Pressed(Key),
    Pasted(String),
}

impl HumanInput {
    pub(crate) fn input(&self) -> Input {
        match self {
            HumanInput::Typed(text) => Input::Type { text: text.clone(), delay_ms: 0 },
            HumanInput::Pressed(key) => Input::Press { keys: vec![*key], repeat: 1 },
            HumanInput::Pasted(text) => Input::Paste { text: text.clone() },
        }
    }

    pub(crate) fn call(&self) -> Vec<String> {
        match self {
            HumanInput::Typed(text) => vec!["type".into(), text.clone()],
            HumanInput::Pressed(key) => vec!["press".into(), key.to_string()],
            HumanInput::Pasted(text) => vec!["paste".into(), text.clone()],
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AttachInput {
    Human(HumanInput),
    Detach,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AttachEvent {
    Screen(Mirror),
    Detached,
    Closed,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Mirror {
    pub program: String,
    pub frame: u64,
    pub cols: u16,
    pub rows: u16,
    pub lines: Vec<String>,
    pub cursor: [u16; 2],
    pub cursor_visible: bool,
    pub exited: bool,
    pub exit_code: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Attached {
    pub session: String,
    pub tab: String,
    pub program: String,
    pub read_only: bool,
}
