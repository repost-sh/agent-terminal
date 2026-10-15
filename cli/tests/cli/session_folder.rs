use std::path::Path;
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

use crate::support::{AgentTerminal, fixture};

fn names_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .unwrap_or_else(|e| panic!("read {}: {e}", folder.display()))
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn casts_in(folder: &Path) -> Vec<String> {
    names_in(folder).into_iter().filter(|name| name.ends_with(".cast")).collect()
}

fn open_printer(at: &AgentTerminal, session: &str, command: &str) {
    at.run(&["--session", session, command, "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
}

#[test]
fn every_tab_of_a_session_records_into_the_session_folder() {
    let at = AgentTerminal::new();
    let folder = at.home().join("sessions").join("qa");
    open_printer(&at, "qa", "open");
    at.run(&["--session", "qa", "tab", "new", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    at.run(&["--session", "qa", "tab", "close", "t1"]).success();
    at.run(&["--session", "qa", "tab", "new", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    assert_eq!(casts_in(&folder), ["t1.cast", "t2.cast", "t3.cast"]);
}

#[test]
fn a_session_that_starts_again_starts_a_fresh_folder() {
    let at = AgentTerminal::new();
    let folder = at.home().join("sessions").join("qa");
    open_printer(&at, "qa", "open");
    at.run(&["--session", "qa", "tab", "new", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    at.run(&["close", "--all"]).success();
    assert_eq!(casts_in(&folder), ["t1.cast", "t2.cast"]);

    open_printer(&at, "qa", "open");
    assert_eq!(casts_in(&folder), ["t1.cast"]);
}

#[test]
fn simultaneous_opens_in_a_new_session_share_one_folder() {
    let at = AgentTerminal::new();
    open_printer(&at, "warm-up", "open");
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| open_printer(&at, "qa", "open"));
        }
    });
    assert_eq!(casts_in(&at.home().join("sessions").join("qa")), ["t1.cast", "t2.cast", "t3.cast", "t4.cast"]);
}

#[test]
fn a_session_name_cannot_lead_its_folder_out_of_the_sessions_folder() {
    let at = AgentTerminal::new();
    open_printer(&at, "../escape/qa", "open");
    assert_eq!(names_in(&at.home().join("sessions")), ["%2E.%2Fescape%2Fqa"]);
    assert!(!at.home().join("escape").exists());
}

#[test]
fn a_session_with_an_empty_name_gets_a_folder_of_its_own() {
    let at = AgentTerminal::new();
    open_printer(&at, "qa", "open");
    open_printer(&at, "", "open");
    assert_eq!(names_in(&at.home().join("sessions")), ["%", "qa"]);
    assert_eq!(casts_in(&at.home().join("sessions").join("qa")), ["t1.cast"]);
}

fn screenshots_listed(at: &AgentTerminal, session: &str) -> Vec<Value> {
    let path = at.home().join("sessions").join(session).join("screenshots.jsonl");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

fn frame(at: &AgentTerminal) -> u64 {
    at.run(&["get", "frame"]).success().stdout().trim().parse().expect("a frame number")
}

#[track_caller]
fn assert_recent_utc_millis(entry: &Value, since: SystemTime) {
    let time = entry["time"].as_str().unwrap_or_else(|| panic!("no time in {entry}"));
    assert_eq!(time.len(), "2026-10-14T14:01:22.051Z".len(), "not a UTC time to the millisecond: {time}");
    let taken = humantime::parse_rfc3339(time).unwrap_or_else(|e| panic!("{time}: {e}"));
    assert!(taken >= since && taken <= SystemTime::now(), "{time} is not the time of this run");
}

#[test]
fn every_png_screenshot_is_listed_in_the_session_folder_with_the_call_that_took_it() {
    let at = AgentTerminal::new();
    let printer = fixture("line-printer");
    let shots = at.home().join("shots");
    let first = shots.join("first.png").to_string_lossy().into_owned();
    let crop = shots.join("crop.png").to_string_lossy().into_owned();
    let text = shots.join("screen.txt").to_string_lossy().into_owned();
    let diff = shots.join("diff.png").to_string_lossy().into_owned();
    let began = SystemTime::now() - Duration::from_secs(1);
    at.run(&["open", "--size", "20x4", "--wait", "line 3", &printer, "--hold"]).success();
    at.run(&["screenshot", &first]).success();
    let first_frame = frame(&at);
    at.run(&["type", "x", "--wait", "x", "--screenshot"]).success();
    let typed_frame = frame(&at);
    at.run(&["screenshot", "--scale", "2", "--rows", "2:3", &crop]).success();
    at.run(&["screenshot", &text]).success();
    assert_eq!(at.run(&["diff", "screenshot", "--baseline", &first, "--output", &diff]).code(), 2);

    let mut listed = screenshots_listed(&at, "default");
    for entry in &mut listed {
        assert_recent_utc_millis(entry, began);
        entry.as_object_mut().unwrap().remove("time");
    }
    let typed = at.home().join("screenshots").join(format!("default-t1-{typed_frame}.png"));
    let full = json!({ "first": 1, "last": 4 });
    assert_eq!(
        listed,
        [
            json!({ "tab": "t1", "frame": first_frame, "path": first, "by": format!("screenshot {first}"),
                "width": 200, "height": 100, "scale": 1, "cols": 20, "rows": full }),
            json!({ "tab": "t1", "frame": typed_frame, "path": typed.to_string_lossy(), "by": "type x --wait x --screenshot",
                "width": 200, "height": 100, "scale": 1, "cols": 20, "rows": full }),
            json!({ "tab": "t1", "frame": typed_frame, "path": crop, "by": format!("screenshot --scale 2 --rows 2:3 {crop}"),
                "width": 400, "height": 120, "scale": 2, "cols": 20, "rows": { "first": 2, "last": 3 } }),
        ]
    );
}
