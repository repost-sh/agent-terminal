use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

use crate::support::{AgentTerminal, fixture};

fn activity(at: &AgentTerminal, session: &str) -> Vec<Value> {
    let path = at.home().join("sessions").join(session).join("activity.jsonl");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

fn field(log: &[Value], name: &str) -> Value {
    log.iter().map(|entry| entry[name].clone()).collect()
}

fn time_of(entry: &Value) -> SystemTime {
    let time = entry["time"].as_str().unwrap_or_else(|| panic!("no time in {entry}"));
    assert_eq!(time.len(), "2026-10-14T14:01:22.051Z".len(), "not a UTC time to the millisecond: {time}");
    humantime::parse_rfc3339(time).unwrap_or_else(|e| panic!("{time}: {e}"))
}

#[test]
fn every_call_is_logged_with_its_command_duration_and_result() {
    let at = AgentTerminal::new();
    let printer = fixture("line-printer");
    let began = SystemTime::now() - Duration::from_secs(1);
    at.run(&["open", "--wait", "line 3", &printer, "--hold"]).success();
    at.run(&["mark", "before-typing"]).success();
    at.run(&["type", "hello world", "--wait", "hello world"]).success();
    let frame: u64 = at.run(&["get", "frame"]).success().stdout().trim().parse().unwrap();
    assert_eq!(at.run(&["get", "cell", "99", "1"]).code(), 1);
    assert_eq!(at.run(&["--timeout", "300", "press", "x", "--wait", "never shown"]).code(), 3);
    at.run(&["close"]).success();
    let ended = SystemTime::now() + Duration::from_secs(1);

    let log = activity(&at, "default");
    assert_eq!(
        field(&log, "command"),
        json!([
            format!("open --wait \"line 3\" {printer} --hold"),
            "mark before-typing",
            "type \"hello world\" --wait \"hello world\"",
            "get frame",
            "get cell 99 1",
            "--timeout 300 press x --wait \"never shown\"",
            "close",
        ])
    );
    assert_eq!(field(&log, "exit"), json!([0, 0, 0, 0, 1, 3, 0]));
    assert_eq!(field(&log, "tab"), json!(["t1", "t1", "t1", "t1", "t1", "t1", "t1"]));
    assert_eq!(log[3]["frame"], frame);
    assert_eq!(log[4]["error"], "cell 99,1 is outside the 120x40 screen");
    assert!(log[5]["error"].as_str().unwrap().starts_with("timed out after 300 ms waiting for \"never shown\""));
    assert!(log[5]["duration_ms"].as_u64().unwrap() >= 300, "{}", log[5]);
    assert!(log.iter().all(|entry| entry["duration_ms"].is_u64()), "{log:?}");
    let times: Vec<SystemTime> = log.iter().map(time_of).collect();
    assert!(times.windows(2).all(|pair| pair[0] <= pair[1]), "{log:?}");
    assert!(began <= times[0] && times[6] <= ended, "{log:?}");
}

#[test]
fn each_session_logs_its_own_calls_and_the_tab_they_acted_on() {
    let at = AgentTerminal::new();
    let printer = fixture("line-printer");
    at.run(&["--session", "qa", "open", "--wait", "line 3", &printer, "--hold"]).success();
    at.run(&["--session", "qa", "tab", "new", "--wait", "line 3", &printer, "--hold"]).success();
    at.run(&["--session", "other", "open", "--wait", "line 3", &printer, "--hold"]).success();
    at.run(&["--session", "qa", "--tab", "t1", "snapshot"]).success();
    at.run(&["--session", "qa", "tab", "close", "t1"]).success();
    at.run(&["--session", "qa", "tab"]).success();

    let log = activity(&at, "qa");
    assert_eq!(field(&log, "tab"), json!(["t1", "t2", "t1", "t1", null]));
    assert_eq!(activity(&at, "other").len(), 1);
}

#[test]
fn calls_that_reach_no_session_leave_no_folder() {
    let at = AgentTerminal::new();
    assert_eq!(at.run(&["--session", "ghost", "snapshot"]).code(), 1);
    assert_eq!(at.run(&["--session", "typo", "open", "no-such-program-xyz"]).code(), 1);
    assert!(!at.home().join("sessions").join("ghost").exists());
    assert!(!at.home().join("sessions").join("typo").exists());
}
