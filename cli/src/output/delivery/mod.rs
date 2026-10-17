use super::count;
use crate::key::Mods;
use crate::protocol::{ClickTarget, Delivered, Delivery, Direction, MouseButton, MouseEvent, ScrollBy};

pub(super) fn delivery(d: &Delivered) -> String {
    let target = format!("{} {}", d.session, d.tab);
    match &d.delivery {
        Delivery::Text { keys, .. } => format!("typed {} into {target}", count(*keys, "key")),
        Delivery::Keys { keys, .. } => format!("pressed {} in {target}", count(*keys, "key")),
        Delivery::Paste { bracketed: true, bytes } => {
            format!("pasted {} into {target} (bracketed paste)", count(*bytes, "byte"))
        }
        Delivery::Paste { bracketed: false, bytes } => {
            format!("pasted {} into {target} (plain, the app has not turned on bracketed paste)", count(*bytes, "byte"))
        }
        Delivery::Raw { bytes } => format!("sent {} to {target}", count(*bytes, "raw byte")),
        Delivery::Signal { signal, process_group } => {
            format!("sent {signal} to {target} (process group {process_group})")
        }
        Delivery::Click { target: clicked, row, col, button, double, mods, .. } => {
            let click = Click { clicked, row: *row, col: *col, button: *button, double: *double, mods: *mods };
            clicked_text(&click, &target)
        }
        Delivery::Mouse { event, bytes } => mouse_text(*event, *bytes, &target),
        Delivery::Scroll { direction, count, by, .. } => {
            let with = match (by, direction) {
                (ScrollBy::Wheel, _) => "the mouse wheel",
                (ScrollBy::Keys, Direction::Up) => "PageUp",
                (ScrollBy::Keys, Direction::Down) => "PageDown",
            };
            format!("scrolled {direction} {count} in {target} with {with}")
        }
        Delivery::Focus { focused, bytes } => {
            let change = if *focused { "in" } else { "out" };
            let note = if *bytes == 0 { " (not sent: the app does not ask for focus reports)" } else { "" };
            format!("reported focus {change} to {target}{note}")
        }
    }
}

struct Click<'a> {
    clicked: &'a ClickTarget,
    row: u16,
    col: u16,
    button: MouseButton,
    double: bool,
    mods: Mods,
}

fn clicked_text(click: &Click<'_>, target: &str) -> String {
    let Click { clicked, row, col, .. } = click;
    let what = match clicked {
        ClickTarget::Cell { .. } => format!("{row},{col}"),
        ClickTarget::Ref(_) => format!("{clicked} at {row},{col}"),
        ClickTarget::Text(text) => format!("{text:?} at {row},{col}"),
    };
    let mut notes = button_note(click.button);
    if click.double {
        notes.push("double click".into());
    }
    notes.extend(held_note(click.mods));
    format!("clicked {what} in {target}{}", noted(&notes))
}

fn mouse_text(event: MouseEvent, bytes: usize, target: &str) -> String {
    let (what, mut notes) = match event {
        MouseEvent::Move { row, col, .. } => (format!("mouse move to {row},{col}"), Vec::new()),
        MouseEvent::Down { row, col, button, .. } => (format!("mouse down at {row},{col}"), button_note(button)),
        MouseEvent::Up { row, col, button, .. } => (format!("mouse up at {row},{col}"), button_note(button)),
        MouseEvent::Wheel { row, col, direction, .. } => {
            (format!("mouse wheel {direction} at {row},{col}"), Vec::new())
        }
    };
    notes.extend(held_note(event.mods()));
    if bytes == 0 {
        notes.push("not sent: the app does not track this event".into());
    }
    format!("{what} in {target}{}", noted(&notes))
}

fn button_note(button: MouseButton) -> Vec<String> {
    match button {
        MouseButton::Left => Vec::new(),
        other => vec![format!("{other} button")],
    }
}

fn held_note(mods: Mods) -> Option<String> {
    (!mods.is_none()).then(|| format!("{mods} held"))
}

fn noted(notes: &[String]) -> String {
    if notes.is_empty() { String::new() } else { format!(" ({})", notes.join(", ")) }
}

#[cfg(test)]
mod tests;
