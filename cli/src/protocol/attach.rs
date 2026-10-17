use serde::{Deserialize, Serialize};

use super::{Input, MouseButton, MouseEvent, Name};
use crate::key::Key;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum HumanInput {
    Typed(String),
    Pressed(Key),
    Pasted(String),
    Mouse(MouseEvent),
    Focused(bool),
}

impl HumanInput {
    pub(crate) fn input(&self) -> Input {
        match self {
            HumanInput::Typed(text) => Input::Type { text: text.clone(), delay_ms: 0 },
            HumanInput::Pressed(key) => Input::Press { keys: vec![*key], repeat: 1 },
            HumanInput::Pasted(text) => Input::Paste { text: text.clone() },
            HumanInput::Mouse(event) => Input::Mouse { event: *event },
            HumanInput::Focused(focused) => Input::Focus { focused: *focused },
        }
    }

    pub(crate) fn call(&self) -> Option<Vec<String>> {
        match self {
            HumanInput::Typed(text) => Some(vec!["type".into(), text.clone()]),
            HumanInput::Pressed(key) => Some(vec!["press".into(), key.to_string()]),
            HumanInput::Pasted(text) => Some(vec!["paste".into(), text.clone()]),
            HumanInput::Mouse(event) => mouse_call(*event),
            HumanInput::Focused(_) => None,
        }
    }
}

fn mouse_call(event: MouseEvent) -> Option<Vec<String>> {
    let (row, col) = event.at();
    let (mut words, button) = match event {
        MouseEvent::Move { .. } => return None,
        MouseEvent::Down { button, .. } => (vec!["down".to_string()], Some(button)),
        MouseEvent::Up { button, .. } => (vec!["up".to_string()], Some(button)),
        MouseEvent::Wheel { direction, .. } => (vec!["wheel".to_string(), direction.to_string()], None),
    };
    words.insert(0, "mouse".into());
    words.push(format!("{row},{col}"));
    words.extend(button.filter(|button| *button != MouseButton::Left).map(|button| format!("--{button}")));
    words.extend(event.mods().names().map(|name| format!("--{}", name.to_ascii_lowercase())));
    Some(words)
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
    pub session: Name,
    pub tab: Name,
    pub program: String,
    pub read_only: bool,
}
