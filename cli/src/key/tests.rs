use super::Key;

fn parsed(texts: &[&str]) -> String {
    texts
        .iter()
        .map(|text| {
            format!(
                "{text:?} -> {}",
                text.parse::<Key>().map_or_else(|refused| format!("refused: {refused}"), |key| key.to_string())
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_key_is_a_name_or_one_character_with_any_modifiers() {
    let keys = [
        "a",
        "A",
        "Ctrl+A",
        "ctrl+shift+f5",
        "Control+x",
        "ALT+Enter",
        "Alt++",
        "+",
        "Shift+Tab",
        "space",
        "PageUp",
        "f12",
        "Esc",
    ];
    insta::assert_snapshot!(parsed(&keys), @r##"
        "a" -> a
        "A" -> A
        "Ctrl+A" -> Ctrl+a
        "ctrl+shift+f5" -> Ctrl+Shift+F5
        "Control+x" -> Ctrl+x
        "ALT+Enter" -> Alt+Enter
        "Alt++" -> Alt++
        "+" -> +
        "Shift+Tab" -> Shift+Tab
        "space" -> Space
        "PageUp" -> PageUp
        "f12" -> F12
        "Esc" -> Esc
    "##);
}

#[test]
fn an_unknown_key_or_modifier_or_a_trailing_plus_is_refused() {
    insta::assert_snapshot!(parsed(&["Ctrl+", "F13", "F0", "Hyper+a", "Enterr", "", "Ctrl+Alt", "+a"]), @r##"
        "Ctrl+" -> refused: unknown key "Ctrl+"; keys are Enter, Tab, Esc, Backspace, Delete, Insert, Up, Down, Left, Right, Home, End, PageUp, PageDown, F1-F12, Space, or a single character, with Ctrl+, Alt+ or Shift+
        "F13" -> refused: unknown key "F13"; keys are Enter, Tab, Esc, Backspace, Delete, Insert, Up, Down, Left, Right, Home, End, PageUp, PageDown, F1-F12, Space, or a single character, with Ctrl+, Alt+ or Shift+
        "F0" -> refused: unknown key "F0"; keys are Enter, Tab, Esc, Backspace, Delete, Insert, Up, Down, Left, Right, Home, End, PageUp, PageDown, F1-F12, Space, or a single character, with Ctrl+, Alt+ or Shift+
        "Hyper+a" -> refused: unknown modifier "Hyper" in "Hyper+a"; use Ctrl+, Alt+ or Shift+
        "Enterr" -> refused: unknown key "Enterr"; keys are Enter, Tab, Esc, Backspace, Delete, Insert, Up, Down, Left, Right, Home, End, PageUp, PageDown, F1-F12, Space, or a single character, with Ctrl+, Alt+ or Shift+
        "" -> refused: unknown key ""; keys are Enter, Tab, Esc, Backspace, Delete, Insert, Up, Down, Left, Right, Home, End, PageUp, PageDown, F1-F12, Space, or a single character, with Ctrl+, Alt+ or Shift+
        "Ctrl+Alt" -> refused: unknown key "Alt"; keys are Enter, Tab, Esc, Backspace, Delete, Insert, Up, Down, Left, Right, Home, End, PageUp, PageDown, F1-F12, Space, or a single character, with Ctrl+, Alt+ or Shift+
        "+a" -> refused: unknown modifier "" in "+a"; use Ctrl+, Alt+ or Shift+
    "##);
}

#[test]
fn text_types_as_one_key_per_character_with_newlines_and_tabs_as_their_keys() {
    let typed = |text: &str| Key::for_text(text).map(|keys| keys.iter().map(ToString::to_string).collect::<Vec<_>>());
    assert_eq!(
        typed("a\nB\r\tc é"),
        Ok(["a", "Enter", "B", "Enter", "Tab", "c", "Space", "é"].map(String::from).to_vec())
    );
    assert_eq!(typed(""), Ok(Vec::new()));
    assert_eq!(
        typed("a\u{1}"),
        Err("cannot type the control character U+0001; press it as a key (Ctrl+c, Esc) or send it with `agent-terminal send`".to_string())
    );
}
