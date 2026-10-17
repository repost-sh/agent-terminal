use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

use crate::support::{AgentTerminal, Run, fixture};

fn field(log: &[Value], name: &str) -> Value {
    log.iter().map(|entry| entry[name].clone()).collect()
}

fn time_of(entry: &Value) -> SystemTime {
    let time = entry["time"].as_str().unwrap_or_else(|| panic!("no time in {entry}"));
    assert_eq!(time.len(), "2026-10-14T14:01:22.051Z".len(), "not a UTC time to the millisecond: {time}");
    humantime::parse_rfc3339(time).unwrap_or_else(|e| panic!("{time}: {e}"))
}

struct Window {
    began: SystemTime,
    ended: SystemTime,
}

#[test]
fn every_call_is_logged_with_its_command_duration_and_result() {
    let at = AgentTerminal::new();
    let printer = fixture("line-printer");
    let (window, frame) = make_one_call_of_each_outcome(&at, &printer);

    let log = at.activity("work");
    the_log_names_each_command_its_exit_and_tab(&log, &printer);
    the_log_keeps_the_frame_and_the_errors(&log, frame);
    the_log_times_each_call_in_order(&log, &window);
}

fn make_one_call_of_each_outcome(at: &AgentTerminal, printer: &str) -> (Window, u64) {
    let began = SystemTime::now() - Duration::from_secs(1);
    at.run(&["open", "--name", "app", "--wait", "line 3", printer, "--hold"]).success();
    at.run(&["mark", "before-typing"]).success();
    at.run(&["type", "hello world", "--wait", "hello world"]).success();
    let frame: u64 = at.run(&["get", "frame"]).success().stdout().trim().parse().unwrap();
    assert_eq!(at.run(&["get", "cell", "99", "1"]).code(), 1);
    assert_eq!(at.run(&["--timeout", "300", "press", "x", "--wait", "never shown"]).code(), 3);
    at.run(&["close"]).success();
    let ended = SystemTime::now() + Duration::from_secs(1);
    (Window { began, ended }, frame)
}

fn the_log_names_each_command_its_exit_and_tab(log: &[Value], printer: &str) {
    assert_eq!(
        field(log, "command"),
        json!([
            format!("open --name app --wait 'line 3' {printer} --hold"),
            "mark before-typing",
            "type 'hello world' --wait 'hello world'",
            "get frame",
            "get cell 99 1",
            "--timeout 300 press x --wait 'never shown'",
            "close",
        ])
    );
    assert_eq!(field(log, "exit"), json!([0, 0, 0, 0, 1, 3, 0]));
    assert_eq!(field(log, "tab"), json!(["app", "app", "app", "app", "app", "app", "app"]));
}

fn the_log_keeps_the_frame_and_the_errors(log: &[Value], frame: u64) {
    assert_eq!(log[3]["frame"], frame);
    assert_eq!(log[4]["error"], "cell 99,1 is outside the 120x40 screen");
    assert!(log[5]["error"].as_str().unwrap().starts_with("timed out after 300 ms waiting for \"never shown\""));
}

fn the_log_times_each_call_in_order(log: &[Value], window: &Window) {
    assert!(log[5]["duration_ms"].as_u64().unwrap() >= 300, "{}", log[5]);
    assert!(log.iter().all(|entry| entry["duration_ms"].is_u64()), "{log:?}");
    let times: Vec<SystemTime> = log.iter().map(time_of).collect();
    assert!(times.windows(2).all(|pair| pair[0] <= pair[1]), "{log:?}");
    assert!(window.began <= times[0] && times[6] <= window.ended, "{log:?}");
}

#[test]
fn raw_bytes_sent_with_send_are_flagged_raw_in_the_activity_log() {
    let at = AgentTerminal::new();
    let echo = fixture("input-echo");
    at.run(&["open", "--name", "app", "--size", "40x4", "--wait", "ready", &echo]).success();
    at.run(&["send", r"\e[A", "--wait", r"\e[A"]).success();
    at.run(&["press", "Up"]).success();
    at.run(&["--json", "batch", r"send 'x\x01'"]).success();

    let log = at.activity("work");
    assert_eq!(
        field(&log, "command"),
        json!([
            format!("open --name app --size 40x4 --wait ready {echo}"),
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
    at.run(&["session", "new", "qa"]).success();
    at.run(&["session", "new", "other"]).success();
    at.run(&["--session", "qa", "open", "--name", "app", "--wait", "line 3", &printer, "--hold"]).success();
    at.run(&["--session", "qa", "tab", "new", "--name", "api", "--wait", "line 3", &printer, "--hold"]).success();
    at.run(&["--session", "other", "open", "--name", "app", "--wait", "line 3", &printer, "--hold"]).success();
    at.run(&["--session", "qa", "--tab", "app", "snapshot"]).success();
    at.run(&["--session", "qa", "tab", "close", "app"]).success();
    at.run(&["--session", "qa", "tab"]).success();

    let log = at.activity("qa");
    assert_eq!(field(&log, "tab"), json!(["app", "api", "app", "app", null]));
    assert_eq!(at.activity("other").len(), 1);
}

#[test]
fn a_batch_logs_each_command_it_ran() {
    let at = AgentTerminal::new();
    let printer = fixture("line-printer");
    at.run(&["open", "--name", "app", "--wait", "line 3", &printer, "--hold"]).success();
    assert_eq!(at.run(&["batch", "--bail", "mark before", "get cell 99 1", "get frame"]).code(), 1);

    let log = at.activity("work");
    assert_eq!(
        field(&log, "command"),
        json!([format!("open --name app --wait 'line 3' {printer} --hold"), "mark before", "get cell 99 1"])
    );
    assert_eq!(field(&log, "exit"), json!([0, 0, 1]));
}

struct Printed {
    opened: Run,
    shown: Run,
    outside: Run,
    timed_out: Run,
}

#[test]
fn each_entry_keeps_what_the_call_printed() {
    let at = AgentTerminal::new();
    let printed = make_calls_that_print(&at);

    let log = at.activity("work");
    the_entries_keep_what_the_successful_calls_printed(&log, &printed);
    the_entries_keep_what_the_failed_calls_printed(&log, &printed);
}

fn make_calls_that_print(at: &AgentTerminal) -> Printed {
    let printer = fixture("line-printer");
    let opened = at.run(&["open", "--name", "app", "--size", "40x6", "--wait", "line 3", &printer, "--hold"]).success();
    at.run(&["wait", "--cursor", "4,1"]).success();
    let shown = at.run(&["snapshot"]).success();
    at.run(&["type", "hi", "--wait", "hi"]).success();
    let outside = at.run(&["get", "cell", "99", "1"]);
    let timed_out = at.run(&["--timeout", "300", "wait", "never shown"]);
    assert_eq!(timed_out.code(), 3);
    Printed { opened, shown, outside, timed_out }
}

fn the_entries_keep_what_the_successful_calls_printed(log: &[Value], printed: &Printed) {
    assert_eq!(
        printed.shown.screen(),
        "work app · line-printer · 40x6 · cursor 4,1 · frame N\n1│line 1\n2│line 2\n3│line 3\n…│ (rows 4-6 blank)\n"
    );
    assert_eq!(log[0]["output"], printed.opened.stdout());
    assert_eq!(log[2]["output"], printed.shown.stdout());
    assert_eq!(log[3]["output"], "typed 2 keys into work app\n");
}

fn the_entries_keep_what_the_failed_calls_printed(log: &[Value], printed: &Printed) {
    assert_eq!(log[4]["output"], "error: cell 99,1 is outside the 40x6 screen\n");
    assert_eq!(log[4]["output"], printed.outside.stderr());
    assert_eq!(log[5]["output"], printed.timed_out.stderr());
    assert!(
        log[5]["output"].as_str().unwrap().starts_with("error: timed out after 300 ms waiting for \"never shown\""),
        "{}",
        log[5]
    );
}

#[test]
fn an_entry_keeps_the_first_64_kib_of_a_long_output() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--wait", "line 20000", &fixture("line-printer"), "--lines", "20000", "--hold"])
        .success();
    let printed = at.run(&["output"]).success().stdout();
    assert!(printed.len() > 128 * 1024, "{} bytes", printed.len());

    let output = at.activity("work")[1]["output"].as_str().unwrap().to_string();
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
    at.run(&["open", "--name", "app", "--size", "20x4", "--wait", "line 3", &printer, "--hold"]).success();
    at.run(&["screenshot", &first.to_string_lossy()]).success();
    at.run(&["type", "x", "--wait", "x", "--screenshot", &second.to_string_lossy()]).success();
    let failed =
        at.run(&["diff", "screenshot", "--baseline", &first.to_string_lossy(), "--output", &diff.to_string_lossy()]);
    at.run(&["get", "frame"]).success();

    assert_eq!(failed.code(), 2);
    let log = at.activity("work");
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
    assert_eq!(at.run(&["--session", "typo", "open", "--name", "app", "no-such-program-xyz"]).code(), 1);
    assert!(!at.home().join("sessions").join("ghost").exists());
    assert!(!at.home().join("sessions").join("typo").exists());
}
