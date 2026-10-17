use crate::key::Mods;
use crate::native::screen::{MouseMode, MouseReporting, MouseTracking};
use crate::protocol::{Direction, MouseButton};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Press(MouseButton),
    Release(MouseButton),
    Move { held: Option<MouseButton> },
    Wheel(Direction),
}

const SHIFT: u32 = 4;
const ALT: u32 = 8;
const CTRL: u32 = 16;
const MOTION: u32 = 32;
const NO_BUTTON: u32 = 3;
const WHEEL_UP: u32 = 64;
const WHEEL_DOWN: u32 = 65;
const X10_LAST: u16 = 223;
const UTF8_LAST: u16 = 2015;

pub(crate) struct Event {
    pub action: Action,
    pub row: u16,
    pub col: u16,
    pub mods: Mods,
}

impl Event {
    pub(crate) fn encode(&self, reporting: MouseReporting) -> Result<Vec<u8>, String> {
        let Self { action, row, col, mods } = *self;
        if !reported(action, reporting.tracking) {
            return Ok(Vec::new());
        }
        let code = modifier_bits(mods)
            + match action {
                Action::Release(_) if reporting.encoding != MouseMode::Sgr => NO_BUTTON,
                Action::Press(button) | Action::Release(button) => button_code(button),
                Action::Move { held } => MOTION + held.map_or(NO_BUTTON, button_code),
                Action::Wheel(Direction::Up) => WHEEL_UP,
                Action::Wheel(Direction::Down) => WHEEL_DOWN,
            };
        match reporting.encoding {
            MouseMode::Sgr => {
                let end = if matches!(action, Action::Release(_)) { 'm' } else { 'M' };
                Ok(format!("\x1b[<{code};{col};{row}{end}").into_bytes())
            }
            MouseMode::X10 => x10(code, row, col),
            MouseMode::Utf8 => utf8(code, row, col),
        }
    }
}

fn x10(code: u32, row: u16, col: u16) -> Result<Vec<u8>, String> {
    reachable(row, col, X10_LAST, MouseMode::X10)?;
    let byte =
        |value: u32| u8::try_from(32 + value).map_err(|e| format!("the X10 mouse encoding cannot carry {value}: {e}"));
    Ok(vec![0x1b, b'[', b'M', byte(code)?, byte(u32::from(col))?, byte(u32::from(row))?])
}

fn utf8(code: u32, row: u16, col: u16) -> Result<Vec<u8>, String> {
    reachable(row, col, UTF8_LAST, MouseMode::Utf8)?;
    let mut bytes = b"\x1b[M".to_vec();
    for value in [code, u32::from(col), u32::from(row)] {
        let c = char::from_u32(32 + value)
            .ok_or_else(|| format!("the UTF-8 mouse encoding cannot carry {value} as a character"))?;
        bytes.extend_from_slice(c.encode_utf8(&mut [0u8; 4]).as_bytes());
    }
    Ok(bytes)
}

fn reported(action: Action, tracking: MouseTracking) -> bool {
    match (action, tracking) {
        (Action::Move { .. }, MouseTracking::Click) => false,
        (Action::Move { held }, MouseTracking::Drag) => held.is_some(),
        _ => true,
    }
}

fn modifier_bits(mods: Mods) -> u32 {
    u32::from(mods.shift) * SHIFT + u32::from(mods.alt) * ALT + u32::from(mods.ctrl) * CTRL
}

fn button_code(button: MouseButton) -> u32 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    }
}

fn reachable(row: u16, col: u16, last: u16, encoding: MouseMode) -> Result<(), String> {
    if row > last || col > last {
        return Err(format!(
            "the app uses the {} mouse encoding, which cannot report cells past row or column {last}",
            encoding.as_str()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
