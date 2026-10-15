use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

use crate::support::{AgentTerminal, fixture};

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

    let log = at.activity("default");
    assert_eq!(
        field(&log, "command"),
        json!([
            format!("open --wait 'line 3' {printer} --hold"),
            "mark before-typing",
            "type 'hello world' --wait 'hello world'",
            "get frame",
            "get cell 99 1",
            "--timeout 300 press x --wait 'never shown'",
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
fn raw_bytes_sent_with_send_are_flagged_raw_in_the_activity_log() {
    let at = AgentTerminal::new();
    let echo = fixture("input-echo");
    at.run(&["open", "--size", "40x4", "--wait", "ready", &echo]).success();
    at.run(&["send", r"\e[A", "--wait", r"\e[A"]).success();
    at.run(&["press", "Up"]).success();
    at.run(&["--json", "batch", r"send 'x\x01'"]).success();

    let log = at.activity("default");
    assert_eq!(
        field(&log, "command"),
        json!([
            format!("open --size 40x4 --wait ready {echo}"),
            r#"send "\\e[A" --wait "\\e[A""#,
            "press Up",
            r#"send "x\\x01""#
        ])
    );
    assert_eq!(field(&log, "raw"), json!([null, true, null, true]));
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

    let log = at.activity("qa");
    assert_eq!(field(&log, "tab"), json!(["t1", "t2", "t1", "t1", null]));
    assert_eq!(at.activity("other").len(), 1);
}

#[test]
fn a_batch_logs_each_command_it_ran() {
    let at = AgentTerminal::new();
    let printer = fixture("line-printer");
    at.run(&["open", "--wait", "line 3", &printer, "--hold"]).success();
    assert_eq!(at.run(&["batch", "--bail", "mark before", "get cell 99 1", "get frame"]).code(), 1);

    let log = at.activity("default");
    assert_eq!(
        field(&log, "command"),
        json!([format!("open --wait 'line 3' {printer} --hold"), "mark before", "get cell 99 1"])
    );
    assert_eq!(field(&log, "exit"), json!([0, 0, 1]));
}

#[test]
fn each_entry_keeps_what_the_call_printed() {
    let at = AgentTerminal::new();
    let printer = fixture("line-printer");
    let opened = at.run(&["open", "--size", "40x6", "--wait", "line 3", &printer, "--hold"]).success();
    at.run(&["type", "hi", "--wait", "hi"]).success();
    let outside = at.run(&["get", "cell", "99", "1"]);
    let timed_out = at.run(&["--timeout", "300", "wait", "never shown"]);
    assert_eq!(timed_out.code(), 3);

    let log = at.activity("default");
    assert_eq!(
        opened.screen(),
        "default t1 · line-printer · 40x6 · cursor 4,1 · frame N\n1│line 1\n2│line 2\n3│line 3\n…│ (rows 4-6 blank)\n"
    );
    assert_eq!(log[0]["output"], opened.stdout());
    assert_eq!(log[1]["output"], "typed 2 keys into default t1\n");
    assert_eq!(log[2]["output"], "error: cell 99,1 is outside the 40x6 screen\n");
    assert_eq!(log[2]["output"], outside.stderr());
    assert_eq!(log[3]["output"], timed_out.stderr());
    assert!(
        log[3]["output"].as_str().unwrap().starts_with("error: timed out after 300 ms waiting for \"never shown\""),
        "{}",
        log[3]
    );
}

#[test]
fn an_entry_keeps_the_first_64_kib_of_a_long_output() {
    let at = AgentTerminal::new();
    at.run(&["open", "--wait", "line 20000", &fixture("line-printer"), "--lines", "20000", "--hold"]).success();
    let printed = at.run(&["output"]).success().stdout();
    assert!(printed.len() > 128 * 1024, "{} bytes", printed.len());

    let output = at.activity("default")[1]["output"].as_str().unwrap().to_string();
    let (kept, note) = output.split_at(64 * 1024);
    assert_eq!(kept, &printed[..64 * 1024]);
    assert_eq!(note, format!("\n… {} more bytes not kept\n", printed.len() - 64 * 1024));
}

#[test]
fn every_screenshot_a_call_saves_is_linked_from_its_entry() {
    let at = AgentTerminal::new();
    let printer = fixture("line-printer");
    let shots = at.home().join("shots");
    let (first, second, diff) = (shots.join("first.png"), shots.join("second.png"), shots.join("diff.png"));
    at.run(&["open", "--size", "20x4", "--wait", "line 3", &printer, "--hold"]).success();
    at.run(&["screenshot", &first.to_string_lossy()]).success();
    at.run(&["type", "x", "--wait", "x", "--screenshot", &second.to_string_lossy()]).success();
    let failed =
        at.run(&["diff", "screenshot", "--baseline", &first.to_string_lossy(), "--output", &diff.to_string_lossy()]);
    at.run(&["get", "frame"]).success();

    assert_eq!(failed.code(), 2);
    let log = at.activity("default");
    assert_eq!(
        field(&log, "screenshots"),
        json!([null, [first.to_string_lossy()], [second.to_string_lossy()], [diff.to_string_lossy()], null])
    );
    assert_eq!(field(&log, "exit"), json!([0, 0, 0, 2, 0]));
    assert_eq!(log[3]["error"], failed.stdout().trim_end());
}

#[test]
fn calls_that_reach_no_session_leave_no_folder() {
    let at = AgentTerminal::new();
    assert_eq!(at.run(&["--session", "ghost", "snapshot"]).code(), 1);
    assert_eq!(at.run(&["--session", "typo", "open", "no-such-program-xyz"]).code(), 1);
    assert!(!at.home().join("sessions").join("ghost").exists());
    assert!(!at.home().join("sessions").join("typo").exists());
}
