use crate::support::{AgentTerminal, fixture};

fn open_input_echo(at: &AgentTerminal, modes: &[&str]) {
    let echo = fixture("input-echo");
    let mut args = vec!["open", "--size", "100x4", "--wait", "ready", echo.as_str()];
    args.extend_from_slice(modes);
    at.run(&args).success();
}

#[test]
fn type_types_text_key_by_key() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &[]);
    let typed = at.run(&["type", "héllo\tworld\n", "--wait", r"h\xc3\xa9llo\tworld\r"]).success();
    assert_eq!(typed.stdout(), "typed 12 keys into default t1\n");
    assert_eq!(at.received("default"), r"h\xc3\xa9llo\tworld\r");
}

#[test]
fn type_with_a_delay_sends_each_key_on_its_own() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &["--reads"]);
    let started = std::time::Instant::now();
    at.run(&["type", "abc", "--delay", "60", "--wait", "a|b|c|"]).success();
    assert!(started.elapsed() >= std::time::Duration::from_millis(120), "took {:?}", started.elapsed());
    assert_eq!(at.received("default"), "a|b|c|");
}

#[test]
fn press_sends_named_keys_in_order() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &[]);
    let expected = r"\r\t\e[Z\e\x7f\e[3~\e[A\e[B\e[C\e[D\e[H\e[F\e[5~\e[6~ \e[2~";
    let keys = [
        "Enter",
        "Tab",
        "Shift+Tab",
        "Esc",
        "Backspace",
        "Delete",
        "Up",
        "Down",
        "Right",
        "Left",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "Space",
        "Insert",
    ];
    let mut args = vec!["press"];
    args.extend(keys);
    args.extend(["--wait", expected]);
    let pressed = at.run(&args).success();
    assert_eq!(pressed.stdout(), "pressed 16 keys in default t1\n");
    assert_eq!(at.received("default"), expected);
}

#[test]
fn press_sends_function_keys() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &[]);
    let expected = r"\eOP\eOQ\eOR\eOS\e[15~\e[17~\e[18~\e[19~\e[20~\e[21~\e[23~\e[24~";
    let mut args = vec!["press", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"];
    args.extend(["--wait", expected]);
    at.run(&args).success();
    assert_eq!(at.received("default"), expected);
}

#[test]
fn press_sends_ctrl_alt_and_shift_chords() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &[]);
    let expected = r"\x03\x03\ex\e\r\e\x01A\e[1;5A\e[15;2~\e[1;3D\x00";
    let mut args = vec![
        "press",
        "Ctrl+c",
        "Ctrl+C",
        "Alt+x",
        "Alt+Enter",
        "Ctrl+Alt+a",
        "Shift+a",
        "Ctrl+Up",
        "Shift+F5",
        "Alt+Left",
        "Ctrl+Space",
    ];
    args.extend(["--wait", expected]);
    at.run(&args).success();
    assert_eq!(at.received("default"), expected);
}

#[test]
fn cursor_keys_follow_application_cursor_mode_when_the_app_turns_it_on() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &["--app-cursor"]);
    let expected = r"\eOA\eOB\eOC\eOD\eOH\eOF\e[1;5A";
    at.run(&["press", "Up", "Down", "Right", "Left", "Home", "End", "Ctrl+Up", "--wait", expected]).success();
    assert_eq!(at.received("default"), expected);
}

#[test]
fn keys_use_the_kitty_keyboard_protocol_when_the_app_turns_it_on() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &["--kitty", "1"]);
    let expected = r"\e[27u\e[99;5u\e[120;3u\e[13;2u\e[9;5u\raA\e[A\e[1;6A\e[13~";
    let mut args = vec![
        "press",
        "Esc",
        "Ctrl+c",
        "Alt+x",
        "Shift+Enter",
        "Ctrl+Tab",
        "Enter",
        "a",
        "Shift+a",
        "Up",
        "Ctrl+Shift+Up",
        "F3",
    ];
    args.extend(["--wait", expected]);
    at.run(&args).success();
    assert_eq!(at.received("default"), expected);
}

#[test]
fn every_kitty_enhancement_shapes_the_keys_the_app_receives() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &["--kitty", "31"]);
    let pressed = r"\e[97:65;2;65u\e[97:65;2:3u\e[13u\e[13;1:3u";
    at.run(&["press", "Shift+a", "Enter", "--wait", pressed]).success();
    let typed = format!(r"{pressed}\e[98;;98u\e[98;1:3u");
    at.run(&["type", "b", "--wait", &typed]).success();
    assert_eq!(at.received("default"), typed);
}

#[test]
fn press_repeats_the_whole_sequence() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &[]);
    let pressed = at.run(&["press", "a", "b", "--repeat", "3", "--wait", "ababab"]).success();
    assert_eq!(pressed.stdout(), "pressed 6 keys in default t1\n");
    assert_eq!(at.received("default"), "ababab");
}

#[test]
fn press_drives_a_full_screen_app() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui")]).success();
    at.run(&["press", "j", "Down", "--wait", "Selected: gamma.txt"]).success();
    at.run(&["press", "k", "--wait", "Selected: beta.txt"]).success();
    assert_eq!(
        at.run(&["snapshot"]).success().stdout().lines().nth(2),
        Some("2││alpha.txt         ││Selected: beta.txt          │")
    );
}

#[test]
fn press_refuses_an_unknown_key_with_exit_code_1() {
    let at = AgentTerminal::new();
    let pressed = at.run(&["press", "Enter", "Foo"]);
    assert_eq!(pressed.code(), 1, "{}", pressed.stderr());
    assert!(
        pressed.stderr().contains(
            "unknown key \"Foo\"; keys are Enter, Tab, Esc, Backspace, Delete, Insert, Up, Down, Left, Right, Home, End, PageUp, PageDown, F1-F12, Space, or a single character, with Ctrl+, Alt+ or Shift+"
        ),
        "{}",
        pressed.stderr()
    );
}

#[test]
fn a_chord_the_app_cannot_receive_is_refused_before_any_key_is_sent() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &[]);
    let pressed = at.run(&["press", "a", "Ctrl+1"]);
    assert_eq!(pressed.code(), 1, "{}", pressed.stderr());
    assert_eq!(
        pressed.stderr(),
        "error: default t1: Ctrl+1 has no code a terminal can send; it needs the Kitty keyboard protocol, which this app has not turned on\n"
    );
    at.run(&["press", "b", "--wait", "b"]).success();
    assert_eq!(at.received("default"), "b");
}

#[test]
fn paste_is_a_bracketed_paste_when_the_app_turned_it_on() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &["--bracketed-paste"]);
    let expected = r"\e[200~one\ntwo\e[201~";
    let pasted = at.run(&["paste", "one\ntwo", "--wait", expected]).success();
    assert_eq!(pasted.stdout(), "pasted 19 bytes into default t1 (bracketed paste)\n");
    assert_eq!(at.received("default"), expected);
}

#[test]
fn a_bracketed_paste_cannot_end_itself_early() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &["--bracketed-paste"]);
    let expected = r"\e[200~a[201~b\e[201~";
    at.run(&["paste", "a\x1b[201~b", "--wait", expected]).success();
    assert_eq!(at.received("default"), expected);
}

#[test]
fn paste_writes_the_text_as_typed_lines_when_the_app_has_no_bracketed_paste() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &[]);
    let expected = r"one\rtwo\rthree";
    let pasted = at.run(&["paste", "one\ntwo\r\nthree", "--wait", expected]).success();
    assert_eq!(pasted.stdout(), "pasted 13 bytes into default t1 (plain, the app has not turned on bracketed paste)\n");
    assert_eq!(at.received("default"), expected);
}

#[test]
fn signal_sends_each_supported_signal_to_the_program() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &[]);
    let mut expected = String::new();
    for (signal, shown) in [
        ("INT", "[INT]"),
        ("TERM", "[TERM]"),
        ("tstp", "[TSTP]"),
        ("CONT", "[CONT]"),
        ("WINCH", "[WINCH]"),
        ("SIGHUP", "[HUP]"),
    ] {
        expected.push_str(shown);
        let sent = at.run(&["signal", signal, "--wait", &expected]).success();
        let name = shown.trim_matches(['[', ']']);
        assert!(sent.stdout().starts_with(&format!("sent {name} to default t1 (process group ")), "{}", sent.stdout());
    }
    assert_eq!(at.received("default"), expected);
}

#[test]
fn signal_reaches_the_foreground_process_group_not_just_the_shell() {
    let at = AgentTerminal::new();
    let script = format!("set -m; {}; printf '\\r\\nafter: %s\\r\\n' \"$?\"; read _", fixture("input-echo"));
    at.run(&["open", "--size", "100x10", "--wait", "ready", "bash", "-c", &script]).success();
    let sent = at.run(&["--json", "signal", "INT", "--wait", "[INT]"]).success().json();
    assert_eq!((&sent["input"], &sent["signal"]), (&serde_json::json!("signal"), &serde_json::json!("INT")));
    assert!(sent["process_group"].as_i64().is_some_and(|group| group > 1), "{sent}");
    at.run(&["signal", "KILL", "--wait", "after: 137"]).success();
    let screen = at.run(&["--json", "snapshot"]).success().json();
    assert_eq!(screen["exited"], false, "the shell must survive a KILL meant for its foreground job: {screen}");
    let lines: Vec<&str> = screen["lines"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect();
    assert!(lines[1].starts_with("[INT]"), "{lines:#?}");
    assert!(lines.contains(&"after: 137"), "{lines:#?}");
}

#[test]
fn signal_refuses_an_unknown_signal_with_exit_code_1() {
    let at = AgentTerminal::new();
    let sent = at.run(&["signal", "USR1"]);
    assert_eq!(sent.code(), 1, "{}", sent.stderr());
    assert!(
        sent.stderr().contains("unknown signal USR1; use INT, TERM, KILL, HUP, TSTP, CONT or WINCH"),
        "{}",
        sent.stderr()
    );
}

#[test]
fn simultaneous_input_calls_never_interleave_their_keys() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &[]);
    std::thread::scope(|scope| {
        let first = scope.spawn(|| at.run(&["type", "aaaa", "--delay", "25"]).code());
        let second = scope.spawn(|| at.run(&["type", "bbbb", "--delay", "25"]).code());
        assert_eq!((first.join().unwrap(), second.join().unwrap()), (0, 0));
    });
    at.run(&["type", "z", "--wait", "z"]).success();
    let line = at.received("default");
    assert!(line == "aaaabbbbz" || line == "bbbbaaaaz", "{line}");
}

#[test]
fn send_writes_raw_bytes_given_with_escapes() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &[]);
    let sent = at.run(&["send", r"a\x01\e[Z\t\\", "--wait", r"a\x01\e[Z\t\\"]).success();
    assert_eq!(sent.stdout(), "sent 7 raw bytes to default t1\n");
    assert_eq!(at.received("default"), r"a\x01\e[Z\t\\");
}

#[test]
fn send_refuses_an_unknown_escape_with_exit_code_1() {
    let at = AgentTerminal::new();
    let sent = at.run(&["send", r"\q"]);
    assert_eq!(sent.code(), 1, "{}", sent.stderr());
    assert!(sent.stderr().contains(r"unknown escape \q; use \e, \xHH, \n, \r, \t, \0 or \\"), "{}", sent.stderr());
}

#[test]
fn input_to_a_program_that_has_exited_fails_with_exit_code_5() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x3", "--no-wait", &fixture("line-printer"), "--lines", "1", "--exit", "7"]).success();
    at.run(&["wait", "--exit"]).success();
    let sent = at.run(&["send", "x"]);
    assert_eq!(sent.code(), 5, "{}", sent.stderr());
    assert_eq!(
        sent.stderr(),
        "error: default t1: line-printer has exited with code 7; nothing more can be sent to it (read its last screen with `agent-terminal snapshot`, run it again with `agent-terminal restart`)\n"
    );
}

#[test]
fn typing_stops_with_exit_code_5_as_soon_as_the_program_exits() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x3", "--wait", "line 1", &fixture("line-printer"), "--lines", "1", "--hold"])
        .success();
    let started = std::time::Instant::now();
    let typed = at.run(&["type", "\nab", "--delay", "10000"]);
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "took {:?}", started.elapsed());
    assert_eq!(typed.code(), 5, "{}", typed.stderr());
    assert!(
        typed.stderr().starts_with("error: default t1: line-printer has exited with code 0; nothing more can be sent"),
        "{}",
        typed.stderr()
    );
}

#[test]
fn waiting_after_input_for_text_that_never_appears_times_out_and_shows_the_screen() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &[]);
    let pressed = at.run(&["--timeout", "300", "press", "a", "--wait", "never shown"]);
    assert_eq!(pressed.code(), 3, "{}", pressed.stderr());
    let stderr = pressed.stderr();
    assert!(
        stderr.starts_with(
            "error: timed out after 300 ms waiting for \"never shown\" to appear; default t1 is still running"
        ),
        "{stderr}"
    );
    assert!(stderr.contains("2│a\n"), "{stderr}");
}

#[test]
fn type_refuses_control_characters_and_sends_nothing() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &[]);
    let typed = at.run(&["type", "a\x1bb"]);
    assert_eq!(typed.code(), 1, "{}", typed.stderr());
    assert_eq!(
        typed.stderr(),
        "error: default t1: cannot type the control character U+001B; press it as a key (Ctrl+c, Esc) or send it with `agent-terminal send`\n"
    );
    at.run(&["type", "c", "--wait", "c"]).success();
    assert_eq!(at.received("default"), "c");
}

#[test]
fn input_with_nothing_open_explains_how_to_start() {
    let at = AgentTerminal::new();
    let pressed = at.run(&["press", "Enter"]);
    assert_eq!(pressed.code(), 1);
    assert_eq!(
        pressed.stderr(),
        "error: no terminal is open in session default; start one with `agent-terminal open <program>`\n"
    );
}

#[test]
fn send_is_marked_as_raw_in_its_json_result() {
    let at = AgentTerminal::new();
    open_input_echo(&at, &[]);
    let sent = at.run(&["--json", "send", r"\r\n", "--wait", r"\r\n"]).success().json();
    assert_eq!(sent, serde_json::json!({ "session": "default", "tab": "t1", "input": "raw", "bytes": 2 }));
}
