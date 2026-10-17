use serde_json::{Value, json};

use super::{action, input_action};
use crate::key::Mods;
use crate::protocol::{ClickTarget, Direction, Input, MouseButton, MouseEvent, Request, Signal};

fn labelled(request: Value) -> Option<String> {
    action(&serde_json::from_value::<Request>(request).unwrap())
}

fn on_tab(mut request: Value) -> Option<String> {
    request["session"] = json!("work");
    request["tab"] = json!(null);
    labelled(request)
}

fn waited(wait: &Value) -> Option<String> {
    on_tab(json!({"command": "wait", "timeout_ms": 1, "wait": wait}))
}

fn expected(expect: &Value) -> Option<String> {
    on_tab(json!({"command": "expect", "expect": expect}))
}

#[test]
fn a_wait_or_expect_reads_as_what_it_waited_for_or_checked() {
    assert_eq!(
        [
            waited(&json!({"until": "elapsed", "ms": 500})),
            waited(&json!({"until": "gone", "text": "busy", "regex": false, "rows": null})),
            waited(&json!({"until": "stable", "quiet_ms": 300})),
            on_tab(json!({"command": "wait-control", "timeout_ms": 1})),
            expected(&json!({"check": "text", "text": "ready", "regex": false, "rows": null})),
            expected(&json!({"check": "gone", "text": "x+", "regex": true, "rows": null})),
            expected(&json!({"check": "exit-code", "code": 2})),
            expected(&json!({"check": "cell", "row": 2, "col": 3, "style": ["bold"]})),
        ],
        [
            Some("waited for 500 ms to pass".into()),
            Some("waited for \"busy\" to disappear".into()),
            Some("waited for the screen to stay unchanged for 300 ms".into()),
            Some("waited for control back".into()),
            Some("expected \"ready\"".into()),
            Some("expected /x+/ to be gone".into()),
            Some("expected exit code 2".into()),
            Some("expected the style at 2,3".into()),
        ]
    );
}

#[test]
fn reading_commands_name_what_they_read() {
    let shot = json!({"path": null, "dir": null, "format": "png", "look": {"cursor": true, "scale": 1, "theme": "dark"}, "rows": null, "annotate": false});
    let options = json!({"ansi": false, "refs": false, "rows": null, "scrollback": "none", "squeeze_blank": false});
    assert_eq!(
        [
            on_tab(json!({"command": "snapshot", "options": options})),
            on_tab(json!({"command": "screenshot", "shot": shot})),
            on_tab(json!({"command": "diff-snapshot", "baseline": null})),
            on_tab(json!({"command": "diff-screenshot", "baseline": "/a.png", "shot": shot, "threshold": 0.1})),
            on_tab(json!({"command": "output", "since": null, "raw": false})),
            on_tab(json!({"command": "history", "last": null, "failed": false})),
            on_tab(json!({"command": "history-show", "id": 4})),
            on_tab(json!({"command": "get", "what": "exit-code"})),
            on_tab(json!({"command": "get", "what": {"cell": {"row": 2, "col": 3}}})),
            on_tab(json!({"command": "find", "text": "foo", "regex": false, "all": false})),
        ],
        [
            "read the screen",
            "took a screenshot",
            "compared the screen with its baseline",
            "compared a screenshot with its baseline",
            "read the output",
            "read the command history",
            "read command 4",
            "read the exit code",
            "read the cell at 2,3",
            "looked for \"foo\"",
        ]
        .map(|label| Some(label.to_string()))
    );
}

#[test]
fn every_fact_has_a_name() {
    let names = ["text", "cursor", "size", "frame", "program", "exit-code", "pid", "title", "cwd", "modes"]
        .map(|what| on_tab(json!({"command": "get", "what": what})).unwrap());
    assert_eq!(
        names,
        [
            "read the text",
            "read the cursor",
            "read the size",
            "read the frame",
            "read the program",
            "read the exit code",
            "read the pid",
            "read the title",
            "read the working directory",
            "read the modes",
        ]
    );
}

#[test]
fn opening_names_the_new_tab_and_session_wide_commands_have_no_label() {
    let size = json!({"cols": 80, "rows": 24});
    let opened = json!({"command": "open", "session": "work", "tab": "notes", "program": "vim", "args": [], "size": size, "cwd": "/", "env": [], "wait": null, "timeout_ms": 1});
    let shell = json!({"command": "shell", "session": "work", "tab": "sh", "shell": "zsh", "program": "zsh", "size": size, "cwd": "/", "env": [], "wait": null, "timeout_ms": 1});
    assert_eq!(
        [
            labelled(opened),
            labelled(shell),
            labelled(json!({"command": "sessions"})),
            labelled(json!({"command": "close", "scope": "every-session"})),
            labelled(json!({"command": "dashboard-stop"})),
        ],
        [Some("opened notes".into()), Some("opened sh".into()), None, None, None]
    );
}

#[test]
fn tab_commands_say_what_they_did() {
    let size = json!({"cols": 80, "rows": 24});
    let wait = json!({"until": "stable", "quiet_ms": 300});
    assert_eq!(
        [
            on_tab(json!({"command": "resize", "size": size, "wait": wait, "timeout_ms": 1})),
            on_tab(json!({"command": "restart", "wait": wait, "timeout_ms": 1})),
            on_tab(json!({"command": "close", "scope": "tab"})),
            labelled(json!({"command": "close", "scope": "session", "session": "work"})),
            labelled(json!({"command": "tabs", "session": "work"})),
            labelled(json!({"command": "switch-tab", "session": "work", "tab": "api"})),
            on_tab(json!({"command": "rename-tab", "name": "web"})),
            on_tab(json!({"command": "mark", "name": "build"})),
            on_tab(json!({"command": "exec", "line": "ls -la", "check": false, "timeout_ms": 1})),
            labelled(json!({"command": "trace-export", "session": "work", "path": "/t.zip"})),
        ],
        [
            "resized to 80x24",
            "restarted",
            "closed the tab",
            "closed the session",
            "listed the tabs",
            "switched to api",
            "renamed the tab to web",
            "marked \"build\"",
            "ran \"ls -la\"",
            "exported a trace",
        ]
        .map(|label| Some(label.to_string()))
    );
}

#[test]
fn an_input_reads_as_what_was_typed_pressed_or_clicked() {
    let ctrl = Mods { ctrl: true, ..Mods::default() };
    let click = |target, button, double, mods| Input::Click { target, button, double, mods };
    let inputs = [
        on_tab(json!({"command": "input", "input": {"kind": "type", "text": "hi", "delay_ms": 0}, "wait": null, "timeout_ms": 1})).unwrap(),
        input_action(&Input::Press { keys: vec!["Enter".parse().unwrap(), "Ctrl+c".parse().unwrap()], repeat: 1 }),
        input_action(&Input::Press { keys: vec!["Down".parse().unwrap()], repeat: 3 }),
        input_action(&Input::Paste { text: "a b".into() }),
        input_action(&Input::Send { bytes: vec![27, 91, 65] }),
        input_action(&Input::Signal { signal: Signal::Int }),
        input_action(&click(ClickTarget::Cell { row: 4, col: 12 }, MouseButton::Left, false, Mods::default())),
        input_action(&click(ClickTarget::Text("Save".into()), MouseButton::Right, true, ctrl)),
        input_action(&click(ClickTarget::Ref(3), MouseButton::Left, false, Mods::default())),
        input_action(&Input::Mouse { event: MouseEvent::Down { row: 2, col: 3, button: MouseButton::Middle, mods: ctrl } }),
        input_action(&Input::Scroll { direction: Direction::Down, count: 3, keys: false }),
        input_action(&Input::Focus { focused: true }),
        input_action(&Input::Focus { focused: false }),
    ];
    assert_eq!(
        inputs,
        [
            "typed \"hi\"",
            "pressed Enter Ctrl+c",
            "pressed Down 3 times",
            "pasted \"a b\"",
            "sent 3 raw bytes",
            "sent INT",
            "clicked 4,12",
            "clicked \"Save\" (right button, double click, Ctrl held)",
            "clicked @t3",
            "mouse down at 2,3 (middle button, Ctrl held)",
            "scrolled down 3",
            "reported focus in",
            "reported focus out",
        ]
    );
}
