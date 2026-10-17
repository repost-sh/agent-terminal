use crate::support::{AgentTerminal, fixture, open_tui};

#[test]
fn get_text_prints_the_screen_without_header_or_row_numbers() {
    let at = AgentTerminal::new();
    at.run(&[
        "open",
        "--name",
        "app",
        "--size",
        "40x10",
        "--wait",
        "line 3",
        &fixture("line-printer"),
        "--lines",
        "3",
        "--hold",
    ])
    .success();
    let run = at.run(&["get", "text"]).success();
    assert_eq!(run.stdout(), "line 1\nline 2\nline 3\n");
}

#[test]
fn get_reports_cursor_size_program_and_frame() {
    let at = AgentTerminal::new();
    at.run(&[
        "open",
        "--name",
        "app",
        "--size",
        "40x10",
        "--wait",
        "line 3",
        &fixture("line-printer"),
        "--lines",
        "3",
        "--hold",
    ])
    .success();
    assert_eq!(at.run(&["get", "cursor"]).success().stdout(), "4,1\n");
    assert_eq!(at.run(&["get", "size"]).success().stdout(), "40x10\n");
    assert_eq!(at.run(&["get", "program"]).success().stdout(), "line-printer\n");
    let frame = at.run(&["--json", "snapshot"]).success().json()["frame"].as_u64().unwrap();
    assert_eq!(at.run(&["get", "frame"]).success().stdout(), format!("{frame}\n"));
    get_json_reports_cursor_size_and_frame(&at, frame);
}

fn get_json_reports_cursor_size_and_frame(at: &AgentTerminal, frame: u64) {
    assert_eq!(
        at.run(&["--json", "get", "cursor"]).success().json(),
        serde_json::json!({ "cursor": { "row": 4, "col": 1, "visible": true } })
    );
    assert_eq!(
        at.run(&["--json", "get", "size"]).success().json(),
        serde_json::json!({ "size": { "cols": 40, "rows": 10 } })
    );
    assert_eq!(at.run(&["--json", "get", "frame"]).success().json(), serde_json::json!({ "frame": frame }));
}

#[test]
fn get_exit_code_says_running_until_the_program_exits() {
    let at = AgentTerminal::new();
    at.run(&["session", "new", "running"]).success();
    at.run(&[
        "--session",
        "running",
        "open",
        "--name",
        "app",
        "--wait",
        "line 1",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--hold",
    ])
    .success();
    assert_eq!(at.run(&["--session", "running", "get", "exit-code"]).success().stdout(), "running\n");
    assert_eq!(
        at.run(&["--session", "running", "--json", "get", "exit-code"]).success().json(),
        serde_json::json!({ "exit_code": { "exited": false, "code": null } })
    );

    at.run(&["open", "--name", "app", "--no-wait", &fixture("line-printer"), "--lines", "1", "--exit", "7"]).success();
    at.run(&["wait", "--exit"]).success();
    assert_eq!(at.run(&["get", "exit-code"]).success().stdout(), "7\n");
    assert_eq!(
        at.run(&["--json", "get", "exit-code"]).success().json(),
        serde_json::json!({ "exit_code": { "exited": true, "code": 7 } })
    );
}

#[test]
fn get_pid_is_the_process_id_of_the_program() {
    let at = AgentTerminal::new();
    let opened = at
        .run(&["open", "--name", "app", "--size", "30x3", "--wait", "pid ", "sh", "-c", "echo \"pid $$\"; read _"])
        .success();
    let shown = opened.stdout().lines().nth(1).unwrap().split_once("│pid ").unwrap().1.to_string();
    assert!(shown.parse::<u32>().is_ok(), "{shown}");
    assert_eq!(at.run(&["get", "pid"]).success().stdout(), format!("{shown}\n"));
    assert_eq!(at.run(&["--json", "get", "pid"]).success().json()["pid"].to_string(), shown);
}

#[test]
fn get_title_is_the_title_the_program_set() {
    let at = AgentTerminal::new();
    at.run(&["session", "new", "untitled"]).success();
    at.run(&[
        "--session",
        "untitled",
        "open",
        "--name",
        "app",
        "--wait",
        "line 1",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--hold",
    ])
    .success();
    assert_eq!(at.run(&["--session", "untitled", "get", "title"]).success().stdout(), "");
    assert_eq!(
        at.run(&["--session", "untitled", "--json", "get", "title"]).success().json(),
        serde_json::json!({ "title": null })
    );

    at.run(&["open", "--name", "app", "--wait", "ready", "sh", "-c", "printf '\\033]2;Build logs\\007ready'; read _"])
        .success();
    assert_eq!(at.run(&["get", "title"]).success().stdout(), "Build logs\n");
    assert_eq!(at.run(&["--json", "get", "title"]).success().json(), serde_json::json!({ "title": "Build logs" }));
}

#[test]
fn get_cwd_is_where_the_program_started_until_it_reports_another() {
    let at = AgentTerminal::new();
    at.run(&["session", "new", "reported"]).success();
    let home = at.home().to_string_lossy().into_owned();
    at.run(&[
        "open",
        "--name",
        "app",
        "--cwd",
        &home,
        "--wait",
        "line 1",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--hold",
    ])
    .success();
    assert_eq!(at.run(&["get", "cwd"]).success().stdout(), format!("{home}\n"));
    assert_eq!(at.run(&["--json", "get", "cwd"]).success().json(), serde_json::json!({ "cwd": home }));

    at.run(&[
        "--session",
        "reported",
        "open",
        "--name",
        "app",
        "--wait",
        "moved",
        "sh",
        "-c",
        "printf '\\033]7;file://build-host/srv/my%%20app\\007moved'; read _",
    ])
    .success();
    assert_eq!(at.run(&["--session", "reported", "get", "cwd"]).success().stdout(), "/srv/my app\n");
}

#[test]
fn get_cursor_says_hidden_when_the_app_hides_it() {
    let at = AgentTerminal::new();
    open_tui(&at);
    assert_eq!(at.run(&["get", "cursor"]).success().stdout(), "hidden\n");
}

#[test]
fn get_cell_shows_the_selected_row_is_inverse() {
    let at = AgentTerminal::new();
    open_tui(&at);
    assert_eq!(at.run(&["get", "cell", "2", "2"]).success().stdout(), "\"a\" fg=default bg=default inverse\n");
    assert_eq!(at.run(&["get", "cell", "3", "2"]).success().stdout(), "\"b\" fg=default bg=default\n");
}

#[test]
fn get_cell_reports_colors_bold_and_underline() {
    let at = AgentTerminal::new();
    let styled = "\\033[1;4;31;44mE\\033[0m \\033[38;5;208mX\\033[38;2;255;136;0mY\\033[38;5;10mZ\\033[0m ready";
    at.run(&[
        "open",
        "--name",
        "app",
        "--size",
        "30x3",
        "--wait",
        "ready",
        "sh",
        "-c",
        &format!("printf '{styled}'; read _"),
    ])
    .success();
    assert_eq!(at.run(&["get", "cell", "1", "1"]).success().stdout(), "\"E\" fg=red bg=blue bold underline\n");
    assert_eq!(at.run(&["get", "cell", "1", "2"]).success().stdout(), "\" \" fg=default bg=default\n");
    assert_eq!(at.run(&["get", "cell", "1", "3"]).success().stdout(), "\"X\" fg=208 bg=default\n");
    assert_eq!(at.run(&["get", "cell", "1", "4"]).success().stdout(), "\"Y\" fg=#ff8800 bg=default\n");
    assert_eq!(at.run(&["get", "cell", "1", "5"]).success().stdout(), "\"Z\" fg=bright-green bg=default\n");
    get_json_cell_names_each_style(&at);
}

fn get_json_cell_names_each_style(at: &AgentTerminal) {
    assert_eq!(
        at.run(&["--json", "get", "cell", "1", "1"]).success().json(),
        serde_json::json!({ "cell": {
            "row": 1,
            "col": 1,
            "char": "E",
            "fg": "red",
            "bg": "blue",
            "bold": true,
            "inverse": false,
            "underline": true,
        } })
    );
}

#[test]
fn get_cell_outside_the_screen_fails() {
    let at = AgentTerminal::new();
    at.run(&[
        "open",
        "--name",
        "app",
        "--size",
        "30x3",
        "--wait",
        "line 1",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--hold",
    ])
    .success();
    let run = at.run(&["get", "cell", "4", "1"]);
    assert_eq!(run.code(), 1);
    assert_eq!(run.stderr(), "error: cell 4,1 is outside the 30x3 screen\n");
    assert_eq!(at.run(&["get", "cell", "1", "31"]).code(), 1);
    assert_eq!(at.run(&["get", "cell", "0", "1"]).code(), 1);
}

#[test]
fn get_modes_starts_with_every_mode_off() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--wait", "line 1", &fixture("line-printer"), "--lines", "1", "--hold"])
        .success();
    assert_eq!(
        at.run(&["get", "modes"]).success().stdout(),
        "alt-screen: off\nmouse: off\nbracketed-paste: off\napp-cursor: off\nfocus-events: off\nkeyboard: legacy\n"
    );
}

#[test]
fn get_modes_reports_the_modes_the_program_turned_on() {
    let at = AgentTerminal::new();
    let modes = "\\033[?1049h\\033[?1002h\\033[?1006h\\033[?2004h\\033[?1h\\033[?1004h\\033[>5u";
    at.run(&["open", "--name", "app", "--wait", "ready", "sh", "-c", &format!("printf '{modes}ready'; read _")])
        .success();
    assert_eq!(
        at.run(&["get", "modes"]).success().stdout(),
        "alt-screen: on\nmouse: drag, sgr\nbracketed-paste: on\napp-cursor: on\nfocus-events: on\nkeyboard: disambiguate, report-alternates\n"
    );
    assert_eq!(
        at.run(&["--json", "get", "modes"]).success().json(),
        serde_json::json!({ "modes": {
            "alt_screen": true,
            "mouse": { "tracking": "drag", "encoding": "sgr" },
            "bracketed_paste": true,
            "app_cursor": true,
            "focus_events": true,
            "keyboard": ["disambiguate", "report-alternates"],
        } })
    );
}
