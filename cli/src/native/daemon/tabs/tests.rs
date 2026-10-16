use serde_json::{Value, json};

use crate::native::daemon::tests::{ECHO, Running, on, open, text};

fn shown(snapshot: &Value) -> String {
    let lines: Vec<&str> = snapshot["lines"].as_array().unwrap().iter().map(|line| line.as_str().unwrap()).collect();
    format!(
        "{} {} {} {}x{} exited {} code {}: {}",
        snapshot["session"].as_str().unwrap(),
        snapshot["tab"].as_str().unwrap(),
        snapshot["program"].as_str().unwrap(),
        snapshot["cols"],
        snapshot["rows"],
        snapshot["exited"],
        snapshot["exit_code"],
        lines.join("|").trim_end_matches('|')
    )
}

fn timed(mut request: Value, timeout_ms: u64) -> Value {
    request["timeout_ms"] = json!(timeout_ms);
    request
}

fn tabs(running: &Running) -> String {
    let listed = running.ok(&json!({ "command": "tabs", "session": "s" }));
    let ids: Vec<&str> = listed["tabs"].as_array().unwrap().iter().map(|tab| tab["tab"].as_str().unwrap()).collect();
    format!("current {} of {}", listed["current"], ids.join(" "))
}

#[test]
fn opening_waits_for_the_screen_it_was_asked_for_or_until_the_program_exits() {
    let running = Running::new();
    let exits = open("printf 'bye\\n'; exit 3", &text("never"));
    assert_eq!(
        [shown(&running.open(ECHO)), shown(&running.ok(&exits))],
        ["s t1 sh 30x4 exited false code null: ready", "s t2 sh 30x4 exited true code 3: bye"]
    );
    assert_eq!(running.ok(&open("read x", &Value::Null))["tab"], "t3");
}

#[test]
fn opening_times_out_with_the_screen_when_what_it_waits_for_never_shows() {
    let running = Running::new();
    let (code, message) = running.refused(&timed(open(ECHO, &text("never")), 200));
    assert_eq!(code, 3);
    insta::assert_snapshot!(message, @r#"timed out after 200 ms waiting for "never" to appear; s t1 is still running (read it with `agent-terminal snapshot`, stop it with `agent-terminal close`)"#);
}

#[test]
fn a_program_that_cannot_start_is_refused() {
    let running = Running::new();
    let mut missing = open(ECHO, &text("ready"));
    missing["program"] = json!("/nope");
    let mut nowhere = open(ECHO, &text("ready"));
    nowhere["cwd"] = json!("/nope/dir");
    assert_eq!(
        [running.refused(&missing), running.refused(&nowhere)],
        [
            (1, "command not found: /nope".to_string()),
            (1, "the working directory /nope/dir does not exist".to_string())
        ]
    );
}

#[test]
fn switching_tabs_picks_the_current_one_and_refuses_one_that_is_not_there() {
    let running = Running::new();
    for _ in 0..3 {
        running.open(ECHO);
    }
    let switched = running.ok(&json!({ "command": "switch-tab", "session": "s", "tab": "t1" }));
    assert_eq!((switched["current"].as_str(), tabs(&running)), (Some("t1"), "current \"t1\" of t1 t2 t3".to_string()));
    assert_eq!(
        running.refused(&json!({ "command": "switch-tab", "session": "s", "tab": "t9" })),
        (1, "no tab t9 in session s; list tabs with `agent-terminal tab`".to_string())
    );
}

#[test]
fn closing_the_current_tab_makes_the_newest_left_current_and_the_last_ends_the_session() {
    let running = Running::new();
    for _ in 0..3 {
        running.open(ECHO);
    }
    running.ok(&json!({ "command": "switch-tab", "session": "s", "tab": "t1" }));
    let close = |tab: Option<&str>| on("close", &json!({ "scope": "tab", "tab": tab }));
    let closed = running.ok(&close(None));
    assert_eq!(
        (closed, tabs(&running)),
        (json!({ "closed": [{ "session": "s", "tab": "t1" }] }), "current \"t3\" of t2 t3".to_string())
    );
    running.ok(&close(Some("t2")));
    assert_eq!(tabs(&running), "current \"t3\" of t3");
    running.ok(&close(None));
    assert_eq!(
        (tabs(&running), running.ok(&json!({ "command": "sessions" }))),
        ("current null of ".to_string(), json!({ "sessions": [] }))
    );
    assert_eq!(
        [running.refused(&close(None)).1, running.refused(&close(Some("t9"))).1],
        [
            "no terminal is open in session s; start one with `agent-terminal open <program>`",
            "no tab t9 in session s; list tabs with `agent-terminal tab`"
        ]
    );
}

fn two_sessions() -> Running {
    let running = Running::new();
    running.open(ECHO);
    let mut elsewhere = open(ECHO, &text("ready"));
    elsewhere["session"] = json!("other");
    running.ok(&elsewhere);
    running.open(ECHO);
    running
}

#[test]
fn closing_a_session_closes_its_tabs_and_leaves_every_other_session_running() {
    let running = two_sessions();
    let close = |session: &str| json!({ "command": "close", "session": session, "tab": null, "scope": "session" });
    insta::assert_snapshot!(running.ok(&close("s")), @r#"{"closed":[{"session":"s","tab":"t1"},{"session":"s","tab":"t2"}]}"#);
    let sessions = running.ok(&json!({ "command": "sessions" }));
    let names: Vec<&Value> =
        sessions["sessions"].as_array().unwrap().iter().map(|session| &session["session"]).collect();
    assert_eq!((names, running.ok(&close("s"))), (vec![&json!("other")], json!({ "closed": [] })));
}

#[test]
fn closing_every_session_closes_every_tab_of_every_session() {
    let running = two_sessions();
    insta::assert_snapshot!(running.ok(&on("close", &json!({ "scope": "every-session" }))), @r#"{"closed":[{"session":"other","tab":"t1"},{"session":"s","tab":"t1"},{"session":"s","tab":"t2"}]}"#);
    assert_eq!(running.ok(&json!({ "command": "sessions" })), json!({ "sessions": [] }));
}

#[test]
fn a_resize_redraws_at_the_new_size_and_is_refused_once_the_program_exited() {
    let running = Running::new();
    running.open(ECHO);
    let resize = |cols: u16, wait: &Value, timeout_ms: u64| {
        on("resize", &json!({ "size": { "cols": cols, "rows": 6 }, "wait": wait, "timeout_ms": timeout_ms }))
    };
    assert_eq!(shown(&running.ok(&resize(40, &text("ready"), 10_000))), "s t1 sh 40x6 exited false code null: ready");
    let timed_out = running.refused(&resize(41, &text("never"), 100));
    assert_eq!(
        (timed_out.0, timed_out.1.split(';').next()),
        (3, Some("timed out after 100 ms waiting for \"never\" to appear"))
    );
    running.ok(&open("exit 4", &json!({ "until": "exit", "code": null })));
    let (code, message) = running.refused(&resize(42, &text("ready"), 100));
    assert_eq!(code, 5);
    insta::assert_snapshot!(message, @"s t2: sh has exited with code 4; nothing more can be sent to it (read its last screen with `agent-terminal snapshot`, run it again with `agent-terminal restart`)");
}

#[test]
fn a_restart_runs_the_program_again_in_the_same_tab_and_carries_its_frame_count_on() {
    let running = Running::new();
    let first = running.open(ECHO);
    let restart = on("restart", &json!({ "wait": text("ready"), "timeout_ms": 10_000 }));
    let again = running.ok(&restart);
    assert_eq!(shown(&again), "s t1 sh 30x4 exited false code null: ready");
    assert!(again["frame"].as_u64() > first["frame"].as_u64(), "{first} {again}");
    assert_eq!(tabs(&running), "current \"t1\" of t1");
}
