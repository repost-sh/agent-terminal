use super::{Action, Event};
use crate::key::Mods;
use crate::native::screen::{MouseMode, MouseReporting, MouseTracking};
use crate::protocol::{Direction, MouseButton};

fn sent(action: Action, (row, col): (u16, u16), tracking: MouseTracking, encoding: MouseMode) -> String {
    held(Mods::default(), action, (row, col), tracking, encoding)
}

fn held(mods: Mods, action: Action, (row, col): (u16, u16), tracking: MouseTracking, encoding: MouseMode) -> String {
    match (Event { action, row, col, mods }).encode(MouseReporting { tracking, encoding }) {
        Ok(bytes) => bytes.escape_ascii().to_string(),
        Err(refused) => format!("refused: {refused}"),
    }
}

const ACTIONS: [Action; 8] = [
    Action::Press(MouseButton::Left),
    Action::Press(MouseButton::Middle),
    Action::Press(MouseButton::Right),
    Action::Release(MouseButton::Right),
    Action::Move { held: None },
    Action::Move { held: Some(MouseButton::Left) },
    Action::Wheel(Direction::Up),
    Action::Wheel(Direction::Down),
];

#[test]
fn sgr_sends_each_button_motion_and_wheel_and_ends_a_release_with_m() {
    let sgr = ACTIONS.map(|action| sent(action, (5, 10), MouseTracking::Motion, MouseMode::Sgr));
    insta::assert_snapshot!(sgr.join("\n"), @r##"
        \x1b[<0;10;5M
        \x1b[<1;10;5M
        \x1b[<2;10;5M
        \x1b[<2;10;5m
        \x1b[<35;10;5M
        \x1b[<32;10;5M
        \x1b[<64;10;5M
        \x1b[<65;10;5M
    "##);
}

#[test]
fn x10_and_utf8_send_each_value_past_32_and_a_release_as_no_button() {
    let encoded = |encoding| ACTIONS.map(|action| sent(action, (5, 10), MouseTracking::Motion, encoding)).join("\n");
    insta::assert_snapshot!(encoded(MouseMode::X10), @r##"
        \x1b[M *%
        \x1b[M!*%
        \x1b[M\"*%
        \x1b[M#*%
        \x1b[MC*%
        \x1b[M@*%
        \x1b[M`*%
        \x1b[Ma*%
    "##);
    insta::assert_snapshot!(encoded(MouseMode::Utf8), @r##"
        \x1b[M *%
        \x1b[M!*%
        \x1b[M\"*%
        \x1b[M#*%
        \x1b[MC*%
        \x1b[M@*%
        \x1b[M`*%
        \x1b[Ma*%
    "##);
}

#[test]
fn x10_reaches_cell_223_and_utf8_cell_2015() {
    let click = Action::Press(MouseButton::Left);
    let at = |cell, encoding| sent(click, cell, MouseTracking::Click, encoding);
    let reach = [
        at((223, 223), MouseMode::X10),
        at((224, 1), MouseMode::X10),
        at((1, 224), MouseMode::X10),
        at((2015, 2015), MouseMode::Utf8),
        at((2016, 1), MouseMode::Utf8),
        at((1, 2016), MouseMode::Utf8),
        at((9999, 9999), MouseMode::Sgr),
    ];
    insta::assert_snapshot!(reach.join("\n"), @r##"
        \x1b[M \xff\xff
        refused: the app uses the x10 mouse encoding, which cannot report cells past row or column 223
        refused: the app uses the x10 mouse encoding, which cannot report cells past row or column 223
        \x1b[M \xdf\xbf\xdf\xbf
        refused: the app uses the utf8 mouse encoding, which cannot report cells past row or column 2015
        refused: the app uses the utf8 mouse encoding, which cannot report cells past row or column 2015
        \x1b[<0;9999;9999M
    "##);
}

#[test]
fn click_tracking_drops_motion_and_drag_tracking_drops_motion_with_no_button_held() {
    let moved = |held, tracking| sent(Action::Move { held }, (1, 1), tracking, MouseMode::Sgr);
    let held = Some(MouseButton::Left);
    assert_eq!(
        [
            moved(held, MouseTracking::Click),
            moved(None, MouseTracking::Drag),
            moved(held, MouseTracking::Drag),
            moved(None, MouseTracking::Motion),
            sent(Action::Release(MouseButton::Right), (1, 1), MouseTracking::Click, MouseMode::Sgr)
        ],
        ["", "", "\\x1b[<32;1;1M", "\\x1b[<35;1;1M", "\\x1b[<2;1;1m"]
    );
}

#[test]
fn shift_alt_and_ctrl_add_4_8_and_16_to_every_code_in_every_encoding() {
    let shift = Mods { shift: true, ..Mods::default() };
    let alt = Mods { alt: true, ..Mods::default() };
    let ctrl = Mods { ctrl: true, ..Mods::default() };
    let all = Mods { shift: true, alt: true, ctrl: true };
    let left = Action::Press(MouseButton::Left);
    let with = |mods, action, encoding| held(mods, action, (5, 10), MouseTracking::Motion, encoding);
    insta::assert_snapshot!([
        with(shift, left, MouseMode::Sgr),
        with(alt, left, MouseMode::Sgr),
        with(ctrl, left, MouseMode::Sgr),
        with(all, Action::Release(MouseButton::Right), MouseMode::Sgr),
        with(ctrl, Action::Move { held: None }, MouseMode::Sgr),
        with(shift, Action::Wheel(Direction::Down), MouseMode::Sgr),
        with(all, left, MouseMode::X10),
        with(all, Action::Release(MouseButton::Left), MouseMode::X10),
        with(ctrl, Action::Wheel(Direction::Up), MouseMode::Utf8),
    ].join("\n"), @r##"
        \x1b[<4;10;5M
        \x1b[<8;10;5M
        \x1b[<16;10;5M
        \x1b[<30;10;5m
        \x1b[<51;10;5M
        \x1b[<69;10;5M
        \x1b[M<*%
        \x1b[M?*%
        \x1b[Mp*%
    "##);
}
