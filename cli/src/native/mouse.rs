use crate::native::screen::{MouseMode, MouseReporting, MouseTracking};
use crate::protocol::{Direction, MouseButton};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Press(MouseButton),
    Release(MouseButton),
    Move { held: Option<MouseButton> },
    Wheel(Direction),
}

const MOTION: u32 = 32;
const NO_BUTTON: u32 = 3;
const WHEEL_UP: u32 = 64;
const WHEEL_DOWN: u32 = 65;
const X10_LAST: u16 = 223;
const UTF8_LAST: u16 = 2015;

pub fn encode(action: Action, row: u16, col: u16, reporting: MouseReporting) -> Result<Vec<u8>, String> {
    if !reported(action, reporting.tracking) {
        return Ok(Vec::new());
    }
    let code = match action {
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
        MouseMode::X10 => {
            reachable(row, col, X10_LAST, reporting.encoding)?;
            Ok([b'\x1b', b'[', b'M', 32 + code as u8, 32 + col as u8, 32 + row as u8].to_vec())
        }
        MouseMode::Utf8 => {
            reachable(row, col, UTF8_LAST, reporting.encoding)?;
            let mut bytes = b"\x1b[M".to_vec();
            for value in [code, u32::from(col), u32::from(row)] {
                let c = char::from_u32(32 + value).expect("values up to 2047 are characters");
                bytes.extend_from_slice(c.encode_utf8(&mut [0u8; 4]).as_bytes());
            }
            Ok(bytes)
        }
    }
}

fn reported(action: Action, tracking: MouseTracking) -> bool {
    match (action, tracking) {
        (Action::Move { .. }, MouseTracking::Click) => false,
        (Action::Move { held }, MouseTracking::Drag) => held.is_some(),
        _ => true,
    }
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
