use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Key {
    pub code: KeyCode,
    pub mods: Mods,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Enter,
    Tab,
    Esc,
    Backspace,
    Delete,
    Insert,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    F(u8),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

const NAMED: &[(&str, KeyCode)] = &[
    ("Enter", KeyCode::Enter),
    ("Return", KeyCode::Enter),
    ("Tab", KeyCode::Tab),
    ("Esc", KeyCode::Esc),
    ("Escape", KeyCode::Esc),
    ("Backspace", KeyCode::Backspace),
    ("Delete", KeyCode::Delete),
    ("Del", KeyCode::Delete),
    ("Insert", KeyCode::Insert),
    ("Up", KeyCode::Up),
    ("Down", KeyCode::Down),
    ("Left", KeyCode::Left),
    ("Right", KeyCode::Right),
    ("Home", KeyCode::Home),
    ("End", KeyCode::End),
    ("PageUp", KeyCode::PageUp),
    ("PageDown", KeyCode::PageDown),
    ("Space", KeyCode::Char(' ')),
];

const KEY_NAMES: &str =
    "Enter, Tab, Esc, Backspace, Delete, Insert, Up, Down, Left, Right, Home, End, PageUp, PageDown, F1-F12, Space";

impl Key {
    pub fn for_text(text: &str) -> Result<Vec<Key>, String> {
        text.chars()
            .map(|c| {
                let code = match c {
                    '\n' | '\r' => KeyCode::Enter,
                    '\t' => KeyCode::Tab,
                    c if c.is_control() => {
                        return Err(format!(
                            "cannot type the control character U+{:04X}; press it as a key (Ctrl+c, Esc) or send it with `agent-terminal send`",
                            c as u32
                        ));
                    }
                    c => KeyCode::Char(c),
                };
                Ok(Key { code, mods: Mods::default() })
            })
            .collect()
    }
}

impl FromStr for Key {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let (modifiers, name) = match text.strip_suffix("++") {
            Some(modifiers) => (Some(modifiers), "+"),
            None => match text.rsplit_once('+') {
                Some((modifiers, name)) if !name.is_empty() => (Some(modifiers), name),
                _ => (None, text),
            },
        };
        let mut mods = Mods::default();
        for modifier in modifiers.map(|m| m.split('+').collect::<Vec<_>>()).unwrap_or_default() {
            match modifier.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => mods.ctrl = true,
                "alt" => mods.alt = true,
                "shift" => mods.shift = true,
                _ => return Err(format!("unknown modifier \"{modifier}\" in \"{text}\"; use Ctrl+, Alt+ or Shift+")),
            }
        }
        let mut code = named(name).ok_or_else(|| {
            format!("unknown key \"{name}\"; keys are {KEY_NAMES}, or a single character, with Ctrl+, Alt+ or Shift+")
        })?;
        if let KeyCode::Char(c) = code
            && mods.ctrl
        {
            code = KeyCode::Char(c.to_ascii_lowercase());
        }
        Ok(Key { code, mods })
    }
}

fn named(name: &str) -> Option<KeyCode> {
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return Some(KeyCode::Char(c));
    }
    if let Some(n) = name.strip_prefix(['F', 'f']).and_then(|n| n.parse().ok()).filter(|n| (1..=12).contains(n)) {
        return Some(KeyCode::F(n));
    }
    NAMED.iter().find(|(known, _)| known.eq_ignore_ascii_case(name)).map(|(_, code)| *code)
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (on, name) in [(self.mods.ctrl, "Ctrl+"), (self.mods.alt, "Alt+"), (self.mods.shift, "Shift+")] {
            if on {
                f.write_str(name)?;
            }
        }
        let name = match self.code {
            KeyCode::Char(' ') => "Space",
            KeyCode::Char(c) => return write!(f, "{c}"),
            KeyCode::F(n) => return write!(f, "F{n}"),
            KeyCode::Enter => "Enter",
            KeyCode::Tab => "Tab",
            KeyCode::Esc => "Esc",
            KeyCode::Backspace => "Backspace",
            KeyCode::Delete => "Delete",
            KeyCode::Insert => "Insert",
            KeyCode::Up => "Up",
            KeyCode::Down => "Down",
            KeyCode::Left => "Left",
            KeyCode::Right => "Right",
            KeyCode::Home => "Home",
            KeyCode::End => "End",
            KeyCode::PageUp => "PageUp",
            KeyCode::PageDown => "PageDown",
        };
        f.write_str(name)
    }
}

impl TryFrom<String> for Key {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        text.parse()
    }
}

impl From<Key> for String {
    fn from(key: Key) -> String {
        key.to_string()
    }
}
