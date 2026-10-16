use serde_json::{Value, json};

use crate::native::daemon::tests::{ECHO, Running, on, open, text, waiting};

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
    let names: Vec<&str> = listed["tabs"].as_array().unwrap().iter().map(|tab| tab["tab"].as_str().unwrap()).collect();
    format!("current {} of {}", listed["current"], names.join(" "))
}

fn rename(tab: Option<&str>, name: &str) -> Value {
    on("rename-tab", &json!({ "tab": tab, "name": name }))
}

#[test]
fn opening_waits_for_the_screen_it_was_asked_for_or_until_the_program_exits() {
    let running = Running::new();
    let exits = waiting(open("bye", "printf 'bye\\n'; exit 3"), text("never"));
    assert_eq!(
        [shown(&running.open("echo", ECHO)), shown(&running.ok(&exits))],
        ["s echo sh 30x4 exited false code null: ready", "s bye sh 30x4 exited true code 3: bye"]
    );
    assert_eq!(running.ok(&waiting(open("reader", "read x"), Value::Null))["tab"], "reader");
}

#[test]
fn opening_times_out_with_the_screen_when_what_it_waits_for_never_shows() {
    let running = Running::new();
    let (code, message) = running.refused(&timed(waiting(open("echo", ECHO), text("never")), 200));
    assert_eq!(code, 3);
    insta::assert_snapshot!(message, @r#"timed out after 200 ms waiting for "never" to appear; s echo is still running (read it with `agent-terminal snapshot`, stop it with `agent-terminal close`)"#);
}

#[test]
fn a_program_that_cannot_start_is_refused() {
    let running = Running::new();
    let mut missing = open("echo", ECHO);
    missing["program"] = json!("/nope");
    let mut nowhere = open("echo", ECHO);
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
fn a_name_held_by_a_tab_of_the_session_is_refused_and_nothing_is_started() {
    let running = Running::new();
    running.open("api", ECHO);
    let mut again = waiting(open("api", "touch started"), Value::Null);
    again["cwd"] = json!(running.user());
    assert_eq!(running.refused(&again), (1, "tab api already runs sh in session s; pick another name".to_string()));
    running.ok(&json!({ "command": "new-session", "session": "other" }));
    let mut elsewhere = open("api", ECHO);
    elsewhere["session"] = json!("other");
    assert_eq!((running.ok(&elsewhere)["tab"].as_str(), running.user().join("started").exists()), (Some("api"), false));
}

#[test]
fn switching_tabs_picks_the_current_one_and_refuses_one_that_is_not_there() {
    let running = Running::new();
    for name in ["app", "api", "web"] {
        running.open(name, ECHO);
    }
    let switched = running.ok(&json!({ "command": "switch-tab", "session": "s", "tab": "app" }));
    assert_eq!(
        (switched["current"].as_str(), tabs(&running)),
        (Some("app"), "current \"app\" of app api web".to_string())
    );
    assert_eq!(
        running.refused(&json!({ "command": "switch-tab", "session": "s", "tab": "t1" })),
        (1, "no tab t1 in session s; list tabs with `agent-terminal tab`".to_string())
    );
}

fn names(listed: &Value) -> Vec<(Value, Value)> {
    let tabs = listed["tabs"].as_array().unwrap().iter();
    tabs.map(|tab| (tab["tab"].clone(), tab.get("renamed_from").cloned().unwrap_or(Value::Null))).collect()
}

#[test]
fn renaming_a_tab_keeps_its_place_and_remembers_its_last_name() {
    let running = Running::new();
    for name in ["app", "api"] {
        running.open(name, ECHO);
    }
    let renamed = running.ok(&rename(None, "web"));
    assert_eq!(
        (&renamed["current"], names(&renamed)),
        (&json!("web"), vec![(json!("app"), Value::Null), (json!("web"), json!("api"))])
    );
    assert_eq!(names(&running.ok(&rename(Some("app"), "app"))), names(&renamed));
    running.ok(&rename(Some("web"), "api"));
    let swapped = running.ok(&rename(Some("app"), "web"));
    assert_eq!(names(&swapped), vec![(json!("web"), json!("app")), (json!("api"), json!("web"))]);
    assert_eq!(
        shown(&running.ok(&on("snapshot", &json!({ "tab": "web" })))),
        "s web sh 30x4 exited false code null: ready"
    );
}

#[test]
fn renaming_refuses_a_name_another_tab_holds_and_a_tab_that_is_not_there() {
    let running = Running::new();
    for name in ["app", "api"] {
        running.open(name, ECHO);
    }
    assert_eq!(
        [running.refused(&rename(Some("app"), "api")), running.refused(&rename(Some("web"), "db"))],
        [
            (1, "tab api already runs sh in session s; pick another name".to_string()),
            (1, "no tab web in session s; list tabs with `agent-terminal tab`".to_string())
        ]
    );
}

#[test]
fn closing_the_current_tab_makes_the_newest_left_current_and_the_last_leaves_the_session_empty() {
    let running = Running::new();
    for name in ["app", "api", "web"] {
        running.open(name, ECHO);
    }
    running.ok(&json!({ "command": "switch-tab", "session": "s", "tab": "app" }));
    let close = |tab: Option<&str>| on("close", &json!({ "scope": "tab", "tab": tab }));
    let closed = running.ok(&close(None));
    assert_eq!(
        (closed, tabs(&running)),
        (json!({ "closed": [{ "session": "s", "tab": "app" }] }), "current \"web\" of api web".to_string())
    );
    running.ok(&close(Some("api")));
    assert_eq!(tabs(&running), "current \"web\" of web");
    running.ok(&close(None));
    assert_eq!(
        (tabs(&running), running.ok(&json!({ "command": "sessions" }))),
        ("current null of ".to_string(), json!({ "sessions": [{ "session": "s", "current": null, "tabs": [] }] }))
    );
    assert_eq!(
        [running.refused(&close(None)).1, running.refused(&close(Some("db"))).1],
        [
            "no terminal is open in session s; start one with `agent-terminal open --name <name> <program>`",
            "no tab db in session s; list tabs with `agent-terminal tab`"
        ]
    );
}

fn two_sessions() -> Running {
    let running = Running::new();
    running.open("app", ECHO);
    running.ok(&json!({ "command": "new-session", "session": "other" }));
    let mut elsewhere = open("app", ECHO);
    elsewhere["session"] = json!("other");
    running.ok(&elsewhere);
    running.open("api", ECHO);
    running
}

#[test]
fn closing_a_session_closes_its_tabs_and_leaves_every_other_session_running() {
    let running = two_sessions();
    let close = |session: &str| json!({ "command": "close", "session": session, "scope": "session" });
    insta::assert_snapshot!(running.ok(&close("s")), @r#"{"closed":[{"session":"s","tab":"app"},{"session":"s","tab":"api"}],"ended":["s"]}"#);
    let sessions = running.ok(&json!({ "command": "sessions" }));
    let names: Vec<&Value> =
        sessions["sessions"].as_array().unwrap().iter().map(|session| &session["session"]).collect();
    assert_eq!(
        (names, running.refused(&close("s"))),
        (vec![&json!("other")], (1, "no session s; create it with `agent-terminal session new s`".to_string()))
    );
}

#[test]
fn closing_every_session_closes_every_tab_of_every_session() {
    let running = two_sessions();
    insta::assert_snapshot!(running.ok(&json!({ "command": "close", "scope": "every-session" })), @r#"{"closed":[{"session":"other","tab":"app"},{"session":"s","tab":"app"},{"session":"s","tab":"api"}],"ended":["other","s"]}"#);
    assert_eq!(running.ok(&json!({ "command": "sessions" })), json!({ "sessions": [] }));
}

#[test]
fn a_resize_redraws_at_the_new_size_and_is_refused_once_the_program_exited() {
    let running = Running::new();
    running.open("echo", ECHO);
    let resize = |cols: u16, wait: &Value, timeout_ms: u64| {
        on("resize", &json!({ "size": { "cols": cols, "rows": 6 }, "wait": wait, "timeout_ms": timeout_ms }))
    };
    assert_eq!(shown(&running.ok(&resize(40, &text("ready"), 10_000))), "s echo sh 40x6 exited false code null: ready");
    let timed_out = running.refused(&resize(41, &text("never"), 100));
    assert_eq!(
        (timed_out.0, timed_out.1.split(';').next()),
        (3, Some("timed out after 100 ms waiting for \"never\" to appear"))
    );
    running.ok(&waiting(open("exits", "exit 4"), json!({ "until": "exit", "code": null })));
    let (code, message) = running.refused(&resize(42, &text("ready"), 100));
    assert_eq!(code, 5);
    insta::assert_snapshot!(message, @"s exits: sh has exited with code 4; nothing more can be sent to it (read its last screen with `agent-terminal snapshot`, run it again with `agent-terminal restart`)");
}

#[test]
fn a_restart_runs_the_program_again_in_the_same_tab_and_carries_its_frame_count_on() {
    let running = Running::new();
    let first = running.open("echo", ECHO);
    running.ok(&rename(None, "web"));
    let restart = on("restart", &json!({ "wait": text("ready"), "timeout_ms": 10_000 }));
    let again = running.ok(&restart);
    assert_eq!(shown(&again), "s web sh 30x4 exited false code null: ready");
    assert!(again["frame"].as_u64() > first["frame"].as_u64(), "{first} {again}");
    assert_eq!(tabs(&running), "current \"web\" of web");
}
