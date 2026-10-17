use serde::{Deserialize, Serialize};

use super::{Input, MouseButton, MouseEvent, Name};
use crate::key::{Key, Mods};

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

    pub(crate) fn call(&self) -> Vec<String> {
        match self {
            HumanInput::Typed(text) => vec!["type".into(), text.clone()],
            HumanInput::Pressed(key) => vec!["press".into(), key.to_string()],
            HumanInput::Pasted(text) => vec!["paste".into(), text.clone()],
            HumanInput::Mouse(event) => mouse_call(*event),
            HumanInput::Focused(focused) => vec!["focus".into(), if *focused { "in" } else { "out" }.into()],
        }
    }

    pub(crate) fn logged(&self) -> bool {
        !matches!(self, HumanInput::Mouse(MouseEvent::Move { .. }) | HumanInput::Focused(_))
    }
}

fn mouse_call(event: MouseEvent) -> Vec<String> {
    let (row, col) = event.at();
    let mut words = vec!["mouse".to_string()];
    let button = match event {
        MouseEvent::Move { .. } => {
            words.push("move".into());
            None
        }
        MouseEvent::Down { button, .. } => {
            words.push("down".into());
            Some(button)
        }
        MouseEvent::Up { button, .. } => {
            words.push("up".into());
            Some(button)
        }
        MouseEvent::Wheel { direction, .. } => {
            words.extend(["wheel".into(), direction.to_string()]);
            None
        }
    };
    words.push(format!("{row},{col}"));
    words.extend(button.filter(|button| *button != MouseButton::Left).map(|button| format!("--{button}")));
    words.extend(held_flags(event.mods()));
    words
}

fn held_flags(mods: Mods) -> impl Iterator<Item = String> {
    [(mods.shift, "--shift"), (mods.alt, "--alt"), (mods.ctrl, "--ctrl")]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, flag)| flag.to_string())
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
