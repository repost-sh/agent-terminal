use crate::key::{Key, KeyCode, Mods};
use crate::native::screen::KeyboardModes;

const FUNCTION_KEY_NUMBERS: [u32; 12] = [11, 12, 13, 14, 15, 17, 18, 19, 20, 21, 23, 24];

const DISAMBIGUATE: u8 = 1;
const EVENT_TYPES: u8 = 2;
const ALTERNATE_KEYS: u8 = 4;
const ALL_KEYS_AS_ESCAPES: u8 = 8;
const ASSOCIATED_TEXT: u8 = 16;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Event {
    Press,
    Release,
}

pub fn paste(text: &str, modes: KeyboardModes) -> Vec<u8> {
    if modes.bracketed_paste {
        [&b"\x1b[200~"[..], text.replace('\x1b', "").as_bytes(), b"\x1b[201~"].concat()
    } else {
        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
    }
}

pub fn encode(key: Key, modes: KeyboardModes) -> Result<Vec<u8>, String> {
    let flags = modes.kitty_flags;
    let mut bytes = if flags & (DISAMBIGUATE | ALL_KEYS_AS_ESCAPES) == 0 {
        legacy(key, modes.app_cursor)?
    } else {
        kitty_press(key, flags)?
    };
    if flags & EVENT_TYPES != 0 && reports_release(key, flags) {
        bytes.extend(kitty(key, flags, Event::Release)?);
    }
    Ok(bytes)
}

fn kitty_press(key: Key, flags: u8) -> Result<Vec<u8>, String> {
    let all_keys = flags & ALL_KEYS_AS_ESCAPES != 0;
    match key.code {
        KeyCode::Char(_) if produces_text(key) && !all_keys => legacy(key, false),
        KeyCode::Enter | KeyCode::Tab | KeyCode::Backspace if key.mods == Mods::default() && !all_keys => {
            legacy(key, false)
        }
        _ => kitty(key, flags, Event::Press),
    }
}

fn reports_release(key: Key, flags: u8) -> bool {
    flags & ALL_KEYS_AS_ESCAPES != 0 || !matches!(key.code, KeyCode::Enter | KeyCode::Tab | KeyCode::Backspace)
}

fn produces_text(key: Key) -> bool {
    matches!(key.code, KeyCode::Char(_)) && !key.mods.ctrl && !key.mods.alt
}

fn kitty(key: Key, flags: u8, event: Event) -> Result<Vec<u8>, String> {
    let mut shift = key.mods.shift;
    let mut alternate = None;
    let mut text = None;
    let (number, final_byte) = match key.code {
        KeyCode::Char(c) => {
            let base = single(c.to_lowercase()).unwrap_or(c);
            shift |= base != c;
            let shifted = if shift { single(base.to_uppercase()).unwrap_or(base) } else { base };
            if flags & ALTERNATE_KEYS != 0 && shifted != base {
                alternate = Some(shifted as u32);
            }
            if event == Event::Press
                && flags & ASSOCIATED_TEXT != 0
                && flags & ALL_KEYS_AS_ESCAPES != 0
                && produces_text(key)
            {
                text = Some(shifted as u32);
            }
            (base as u32, b'u')
        }
        KeyCode::Enter => (13, b'u'),
        KeyCode::Tab => (9, b'u'),
        KeyCode::Backspace => (127, b'u'),
        KeyCode::Esc => (27, b'u'),
        KeyCode::Insert => (2, b'~'),
        KeyCode::Delete => (3, b'~'),
        KeyCode::PageUp => (5, b'~'),
        KeyCode::PageDown => (6, b'~'),
        KeyCode::Up => (1, b'A'),
        KeyCode::Down => (1, b'B'),
        KeyCode::Right => (1, b'C'),
        KeyCode::Left => (1, b'D'),
        KeyCode::Home => (1, b'H'),
        KeyCode::End => (1, b'F'),
        KeyCode::F(1) => (1, b'P'),
        KeyCode::F(2) => (1, b'Q'),
        KeyCode::F(4) => (1, b'S'),
        KeyCode::F(n) => (function_key_number(n)?, b'~'),
    };
    let mods = u8::from(shift) + 2 * u8::from(key.mods.alt) + 4 * u8::from(key.mods.ctrl);
    let mods_field = mods != 0 || event == Event::Release;

    let mut out = String::from("\x1b[");
    if number != 1 || alternate.is_some() || mods_field || text.is_some() {
        out.push_str(&number.to_string());
    }
    if let Some(alternate) = alternate {
        out.push_str(&format!(":{alternate}"));
    }
    if mods_field || text.is_some() {
        out.push(';');
    }
    if mods_field {
        out.push_str(&(mods + 1).to_string());
    }
    if event == Event::Release {
        out.push_str(":3");
    }
    if let Some(text) = text {
        out.push_str(&format!(";{text}"));
    }
    out.push(final_byte as char);
    Ok(out.into_bytes())
}

fn single(mut chars: impl Iterator<Item = char>) -> Option<char> {
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

fn legacy(key: Key, app_cursor: bool) -> Result<Vec<u8>, String> {
    let Mods { shift, alt, ctrl } = key.mods;
    let modifier = 1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(ctrl);
    let with_alt = |bytes: &[u8]| [if alt { &b"\x1b"[..] } else { &[] }, bytes].concat();
    Ok(match key.code {
        KeyCode::Char(c) if ctrl => with_alt(&[control_byte(c).ok_or_else(|| {
            format!("{key} has no code a terminal can send; it needs the Kitty keyboard protocol, which this app has not turned on")
        })?]),
        KeyCode::Char(c) => with_alt(if shift { c.to_ascii_uppercase() } else { c }.encode_utf8(&mut [0u8; 4]).as_bytes()),
        KeyCode::Enter => with_alt(b"\r"),
        KeyCode::Tab if shift => with_alt(b"\x1b[Z"),
        KeyCode::Tab => with_alt(b"\t"),
        KeyCode::Esc => with_alt(b"\x1b"),
        KeyCode::Backspace if ctrl => with_alt(b"\x08"),
        KeyCode::Backspace => with_alt(b"\x7f"),
        KeyCode::Up => letter(b'A', modifier, app_cursor),
        KeyCode::Down => letter(b'B', modifier, app_cursor),
        KeyCode::Right => letter(b'C', modifier, app_cursor),
        KeyCode::Left => letter(b'D', modifier, app_cursor),
        KeyCode::Home => letter(b'H', modifier, app_cursor),
        KeyCode::End => letter(b'F', modifier, app_cursor),
        KeyCode::Insert => tilde(2, modifier),
        KeyCode::Delete => tilde(3, modifier),
        KeyCode::PageUp => tilde(5, modifier),
        KeyCode::PageDown => tilde(6, modifier),
        KeyCode::F(n @ 1..=4) => letter(b"PQRS"[usize::from(n) - 1], modifier, true),
        KeyCode::F(n) => tilde(function_key_number(n)?, modifier),
    })
}

fn letter(final_byte: u8, modifier: u8, ss3: bool) -> Vec<u8> {
    match (modifier, ss3) {
        (1, true) => vec![0x1b, b'O', final_byte],
        (1, false) => vec![0x1b, b'[', final_byte],
        _ => format!("\x1b[1;{modifier}{}", final_byte as char).into_bytes(),
    }
}

fn tilde(number: u32, modifier: u8) -> Vec<u8> {
    match modifier {
        1 => format!("\x1b[{number}~").into_bytes(),
        _ => format!("\x1b[{number};{modifier}~").into_bytes(),
    }
}

fn function_key_number(n: u8) -> Result<u32, String> {
    usize::from(n)
        .checked_sub(1)
        .and_then(|i| FUNCTION_KEY_NUMBERS.get(i))
        .copied()
        .ok_or_else(|| format!("there is no F{n} key; function keys are F1-F12"))
}

fn control_byte(c: char) -> Option<u8> {
    match c {
        'a'..='z' | 'A'..='Z' => Some(c.to_ascii_lowercase() as u8 & 0x1f),
        ' ' | '@' | '2' => Some(0),
        '[' | '3' => Some(0x1b),
        '\\' | '4' => Some(0x1c),
        ']' | '5' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '/' | '7' => Some(0x1f),
        '?' | '8' => Some(0x7f),
        _ => None,
    }
}
