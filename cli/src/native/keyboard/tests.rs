use super::{encode, paste};
use crate::key::{Key, KeyCode, Mods};
use crate::native::screen::KeyboardModes;

fn sent(key: &str, modes: KeyboardModes) -> String {
    let key: Key = key.parse().unwrap();
    encode(key, modes)
        .map_or_else(|refused| format!("refused: {refused}"), |bytes| format!("{:?}", String::from_utf8_lossy(&bytes)))
}

fn table(keys: &[&str], modes: KeyboardModes) -> String {
    keys.iter().map(|key| format!("{key} {}", sent(key, modes))).collect::<Vec<_>>().join("\n")
}

const fn kitty(flags: u8) -> KeyboardModes {
    KeyboardModes { app_cursor: false, bracketed_paste: false, kitty_flags: flags }
}

const LEGACY: KeyboardModes = kitty(0);

const KITTY_KEYS: [&str; 20] = [
    "a",
    "A",
    "shift+a",
    "ctrl+a",
    "alt+a",
    "ctrl+shift+a",
    "Enter",
    "shift+Enter",
    "Tab",
    "Backspace",
    "Escape",
    "Up",
    "ctrl+Up",
    "Home",
    "F1",
    "F3",
    "F5",
    "Delete",
    "é",
    "É",
];

#[test]
fn legacy_keys_send_the_xterm_bytes_with_alt_as_an_escape_and_modifiers_as_a_parameter() {
    insta::assert_snapshot!(table(&["a", "A", "shift+a", "alt+a", "ctrl+a", "ctrl+alt+a", "ctrl+space", "ctrl+[", "ctrl+?", "ctrl+1", "é", "Enter", "alt+Enter", "Tab", "shift+Tab", "Escape", "Backspace", "ctrl+Backspace", "Up", "shift+Up", "ctrl+alt+Left", "Home", "End", "Insert", "Delete", "PageUp", "shift+PageDown", "F1", "F3", "ctrl+F4", "F5", "F12", "shift+F10"], LEGACY), @r#"
    a "a"
    A "A"
    shift+a "A"
    alt+a "\u{1b}a"
    ctrl+a "\u{1}"
    ctrl+alt+a "\u{1b}\u{1}"
    ctrl+space "\0"
    ctrl+[ "\u{1b}"
    ctrl+? "\u{7f}"
    ctrl+1 refused: Ctrl+1 has no code a terminal can send; it needs the Kitty keyboard protocol, which this app has not turned on
    é "é"
    Enter "\r"
    alt+Enter "\u{1b}\r"
    Tab "\t"
    shift+Tab "\u{1b}[Z"
    Escape "\u{1b}"
    Backspace "\u{7f}"
    ctrl+Backspace "\u{8}"
    Up "\u{1b}[A"
    shift+Up "\u{1b}[1;2A"
    ctrl+alt+Left "\u{1b}[1;7D"
    Home "\u{1b}[H"
    End "\u{1b}[F"
    Insert "\u{1b}[2~"
    Delete "\u{1b}[3~"
    PageUp "\u{1b}[5~"
    shift+PageDown "\u{1b}[6;2~"
    F1 "\u{1b}OP"
    F3 "\u{1b}OR"
    ctrl+F4 "\u{1b}[1;5S"
    F5 "\u{1b}[15~"
    F12 "\u{1b}[24~"
    shift+F10 "\u{1b}[21;2~"
    "#);
}

#[test]
fn control_with_a_letter_digit_or_bracket_sends_its_control_byte() {
    insta::assert_snapshot!(table(&["ctrl+@", "ctrl+2", "ctrl+3", "ctrl+4", "ctrl+\\", "ctrl+5", "ctrl+]", "ctrl+6", "ctrl+^", "ctrl+7", "ctrl+_", "ctrl+/", "ctrl+8", "ctrl+Z", "ctrl+shift+b", "shift+é"], LEGACY), @r#"
    ctrl+@ "\0"
    ctrl+2 "\0"
    ctrl+3 "\u{1b}"
    ctrl+4 "\u{1c}"
    ctrl+\ "\u{1c}"
    ctrl+5 "\u{1d}"
    ctrl+] "\u{1d}"
    ctrl+6 "\u{1e}"
    ctrl+^ "\u{1e}"
    ctrl+7 "\u{1f}"
    ctrl+_ "\u{1f}"
    ctrl+/ "\u{1f}"
    ctrl+8 "\u{7f}"
    ctrl+Z "\u{1a}"
    ctrl+shift+b "\u{2}"
    shift+é "é"
    "#);
}

#[test]
fn application_cursor_mode_sends_unmodified_arrows_home_and_end_as_ss3() {
    insta::assert_snapshot!(table(&["Up", "Down", "Right", "Left", "Home", "End", "shift+Up"], KeyboardModes { app_cursor: true, ..LEGACY }), @r#"
    Up "\u{1b}OA"
    Down "\u{1b}OB"
    Right "\u{1b}OC"
    Left "\u{1b}OD"
    Home "\u{1b}OH"
    End "\u{1b}OF"
    shift+Up "\u{1b}[1;2A"
    "#);
}

#[test]
fn disambiguation_keeps_text_and_plain_enter_tab_backspace_and_escapes_the_rest() {
    insta::assert_snapshot!(table(&KITTY_KEYS, kitty(1)), @r#"
    a "a"
    A "A"
    shift+a "A"
    ctrl+a "\u{1b}[97;5u"
    alt+a "\u{1b}[97;3u"
    ctrl+shift+a "\u{1b}[97;6u"
    Enter "\r"
    shift+Enter "\u{1b}[13;2u"
    Tab "\t"
    Backspace "\u{7f}"
    Escape "\u{1b}[27u"
    Up "\u{1b}[A"
    ctrl+Up "\u{1b}[1;5A"
    Home "\u{1b}[H"
    F1 "\u{1b}[P"
    F3 "\u{1b}[13~"
    F5 "\u{1b}[15~"
    Delete "\u{1b}[3~"
    é "é"
    É "É"
    "#);
}

#[test]
fn event_types_add_a_release_after_every_key_but_plain_enter_tab_and_backspace() {
    insta::assert_snapshot!(table(&KITTY_KEYS, kitty(3)), @r#"
    a "a\u{1b}[97;1:3u"
    A "A\u{1b}[97;2:3u"
    shift+a "A\u{1b}[97;2:3u"
    ctrl+a "\u{1b}[97;5u\u{1b}[97;5:3u"
    alt+a "\u{1b}[97;3u\u{1b}[97;3:3u"
    ctrl+shift+a "\u{1b}[97;6u\u{1b}[97;6:3u"
    Enter "\r"
    shift+Enter "\u{1b}[13;2u"
    Tab "\t"
    Backspace "\u{7f}"
    Escape "\u{1b}[27u\u{1b}[27;1:3u"
    Up "\u{1b}[A\u{1b}[1;1:3A"
    ctrl+Up "\u{1b}[1;5A\u{1b}[1;5:3A"
    Home "\u{1b}[H\u{1b}[1;1:3H"
    F1 "\u{1b}[P\u{1b}[1;1:3P"
    F3 "\u{1b}[13~\u{1b}[13;1:3~"
    F5 "\u{1b}[15~\u{1b}[15;1:3~"
    Delete "\u{1b}[3~\u{1b}[3;1:3~"
    é "é\u{1b}[233;1:3u"
    É "É\u{1b}[233;2:3u"
    "#);
}

#[test]
fn alternate_keys_name_the_shifted_letter_beside_the_base_one() {
    insta::assert_snapshot!(table(&KITTY_KEYS, kitty(5)), @r#"
    a "a"
    A "A"
    shift+a "A"
    ctrl+a "\u{1b}[97;5u"
    alt+a "\u{1b}[97;3u"
    ctrl+shift+a "\u{1b}[97:65;6u"
    Enter "\r"
    shift+Enter "\u{1b}[13;2u"
    Tab "\t"
    Backspace "\u{7f}"
    Escape "\u{1b}[27u"
    Up "\u{1b}[A"
    ctrl+Up "\u{1b}[1;5A"
    Home "\u{1b}[H"
    F1 "\u{1b}[P"
    F3 "\u{1b}[13~"
    F5 "\u{1b}[15~"
    Delete "\u{1b}[3~"
    é "é"
    É "É"
    "#);
}

#[test]
fn all_keys_as_escapes_sends_every_key_as_its_code() {
    insta::assert_snapshot!(table(&KITTY_KEYS, kitty(8)), @r#"
    a "\u{1b}[97u"
    A "\u{1b}[97;2u"
    shift+a "\u{1b}[97;2u"
    ctrl+a "\u{1b}[97;5u"
    alt+a "\u{1b}[97;3u"
    ctrl+shift+a "\u{1b}[97;6u"
    Enter "\u{1b}[13u"
    shift+Enter "\u{1b}[13;2u"
    Tab "\u{1b}[9u"
    Backspace "\u{1b}[127u"
    Escape "\u{1b}[27u"
    Up "\u{1b}[A"
    ctrl+Up "\u{1b}[1;5A"
    Home "\u{1b}[H"
    F1 "\u{1b}[P"
    F3 "\u{1b}[13~"
    F5 "\u{1b}[15~"
    Delete "\u{1b}[3~"
    é "\u{1b}[233u"
    É "\u{1b}[233;2u"
    "#);
}

#[test]
fn associated_text_follows_the_code_of_a_text_key() {
    insta::assert_snapshot!(table(&KITTY_KEYS, kitty(24)), @r#"
    a "\u{1b}[97;;97u"
    A "\u{1b}[97;2;65u"
    shift+a "\u{1b}[97;2;65u"
    ctrl+a "\u{1b}[97;5u"
    alt+a "\u{1b}[97;3u"
    ctrl+shift+a "\u{1b}[97;6u"
    Enter "\u{1b}[13u"
    shift+Enter "\u{1b}[13;2u"
    Tab "\u{1b}[9u"
    Backspace "\u{1b}[127u"
    Escape "\u{1b}[27u"
    Up "\u{1b}[A"
    ctrl+Up "\u{1b}[1;5A"
    Home "\u{1b}[H"
    F1 "\u{1b}[P"
    F3 "\u{1b}[13~"
    F5 "\u{1b}[15~"
    Delete "\u{1b}[3~"
    é "\u{1b}[233;;233u"
    É "\u{1b}[233;2;201u"
    "#);
}

#[test]
fn every_enhancement_together_sends_codes_alternates_text_and_releases() {
    insta::assert_snapshot!(table(&KITTY_KEYS, kitty(31)), @r#"
    a "\u{1b}[97;;97u\u{1b}[97;1:3u"
    A "\u{1b}[97:65;2;65u\u{1b}[97:65;2:3u"
    shift+a "\u{1b}[97:65;2;65u\u{1b}[97:65;2:3u"
    ctrl+a "\u{1b}[97;5u\u{1b}[97;5:3u"
    alt+a "\u{1b}[97;3u\u{1b}[97;3:3u"
    ctrl+shift+a "\u{1b}[97:65;6u\u{1b}[97:65;6:3u"
    Enter "\u{1b}[13u\u{1b}[13;1:3u"
    shift+Enter "\u{1b}[13;2u\u{1b}[13;2:3u"
    Tab "\u{1b}[9u\u{1b}[9;1:3u"
    Backspace "\u{1b}[127u\u{1b}[127;1:3u"
    Escape "\u{1b}[27u\u{1b}[27;1:3u"
    Up "\u{1b}[A\u{1b}[1;1:3A"
    ctrl+Up "\u{1b}[1;5A\u{1b}[1;5:3A"
    Home "\u{1b}[H\u{1b}[1;1:3H"
    F1 "\u{1b}[P\u{1b}[1;1:3P"
    F3 "\u{1b}[13~\u{1b}[13;1:3~"
    F5 "\u{1b}[15~\u{1b}[15;1:3~"
    Delete "\u{1b}[3~\u{1b}[3;1:3~"
    é "\u{1b}[233;;233u\u{1b}[233;1:3u"
    É "\u{1b}[233:201;2;201u\u{1b}[233:201;2:3u"
    "#);
}

#[test]
fn a_function_key_past_twelve_is_refused() {
    let key = Key { code: KeyCode::F(13), mods: Mods::default() };
    let no_key = Key { code: KeyCode::F(0), mods: Mods::default() };
    let refusal = "there is no F13 key; function keys are F1-F12".to_string();
    assert_eq!(
        [encode(key, LEGACY), encode(no_key, kitty(1))],
        [Err(refusal), Err("there is no F0 key; function keys are F1-F12".to_string())]
    );
}

#[test]
fn a_paste_is_bracketed_without_escapes_or_sent_with_newlines_as_returns() {
    assert_eq!(paste("a\r\nb\nc", LEGACY), b"a\rb\rc");
    assert_eq!(paste("a\x1bb\nc", KeyboardModes { bracketed_paste: true, ..LEGACY }), b"\x1b[200~ab\nc\x1b[201~");
}
