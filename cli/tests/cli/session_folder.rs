use std::path::Path;
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

use crate::support::{AgentTerminal, DashboardSocket, dashboard, fixture};

fn names_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .unwrap_or_else(|e| panic!("read {}: {e}", folder.display()))
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn bytes_in(folder: &Path) -> u64 {
    std::fs::read_dir(folder)
        .unwrap_or_else(|e| panic!("read {}: {e}", folder.display()))
        .map(|entry| entry.unwrap().metadata().expect("a file's size").len())
        .sum()
}

fn open_printer(at: &AgentTerminal, session: &str, command: &str) {
    at.run(&["--session", session, command, "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
}

#[test]
fn no_tab_of_a_session_leaves_a_file_of_its_own_in_the_session_folder() {
    let at = AgentTerminal::new();
    let folder = at.home().join("sessions").join("qa");
    open_printer(&at, "qa", "open");
    at.run(&["--session", "qa", "tab", "new", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    at.run(&["--session", "qa", "tab", "close", "t1"]).success();
    at.run(&["--session", "qa", "tab", "new", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    assert_eq!(names_in(&folder), ["activity.jsonl"]);
}

#[test]
fn a_session_that_starts_again_starts_a_fresh_folder() {
    let at = AgentTerminal::new();
    let folder = at.home().join("sessions").join("qa");
    let shot = at.work_folder().join("first-run.png").to_string_lossy().into_owned();
    open_printer(&at, "qa", "open");
    at.run(&["--session", "qa", "screenshot", &shot]).success();
    at.run(&["close", "--all"]).success();
    assert_eq!(names_in(&folder), ["activity.jsonl", "screenshots.jsonl"]);

    open_printer(&at, "qa", "open");
    assert_eq!(names_in(&folder), ["activity.jsonl"]);
    assert_eq!(at.activity("qa").len(), 1);
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
    assert_eq!(names_in(&at.home().join("sessions").join("qa")), ["activity.jsonl"]);
    assert_eq!(at.activity("qa").len(), 4);
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
    assert_eq!(at.activity("qa").len(), 1);
}

#[test]
fn record_start_and_stop_are_unknown_commands_and_the_home_gets_no_recordings_folder() {
    let at = AgentTerminal::new();
    open_printer(&at, "qa", "open");
    for action in ["start", "stop"] {
        let refused = at.run(&["--session", "qa", "record", action]);
        assert_eq!(refused.code(), 1, "record {action}: {}", refused.stdout());
        assert!(
            refused.stderr().starts_with("error: unrecognized subcommand 'record'\n"),
            "record {action}: {}",
            refused.stderr()
        );
    }
    at.run(&["--session", "qa", "type", "x", "--wait", "x"]).success();
    at.run(&["--session", "qa", "close"]).success();
    assert_eq!(names_in(&at.home().join("sessions").join("qa")), ["activity.jsonl"]);
    assert!(!at.home().join("recordings").exists());
}

fn frame_of(at: &AgentTerminal, session: &str) -> u64 {
    at.run(&["--session", session, "get", "frame"]).success().stdout().trim().parse().expect("a frame number")
}

fn open_redraw(at: &AgentTerminal, session: &str, repaint: &str) {
    at.run(&[
        "--session",
        session,
        "open",
        "--size",
        "120x40",
        "--wait",
        "ready",
        &fixture("redraw"),
        "--hz",
        "60",
        repaint,
    ])
    .success();
}

fn watch_one_frame(viewer: &mut DashboardSocket) -> u64 {
    let message = viewer.next();
    assert_eq!(message["type"], "frame", "{message}");
    let shown = message["frame"].as_u64().expect("a frame number");
    viewer.send(&json!({ "type": "shown", "frame": shown }));
    shown
}

#[test]
fn a_program_repainting_fast_adds_nothing_to_the_session_folder() {
    let at = AgentTerminal::new();
    let folder = at.home().join("sessions").join("qa");
    let shot = at.work_folder().join("fast.png").to_string_lossy().into_owned();
    open_redraw(&at, "qa", "--full");
    let opened = frame_of(&at, "qa");
    at.run(&["--session", "qa", "wait", "--frame", &(opened + 120).to_string()]).success();
    at.run(&["--session", "qa", "screenshot", &shot]).success();
    assert_eq!(names_in(&folder), ["activity.jsonl", "screenshots.jsonl"]);

    let port = dashboard(&at);
    let mut viewer = DashboardSocket::open(port, "/api/session/qa/t1/stream");
    let before = bytes_in(&folder);
    let from = watch_one_frame(&mut viewer);
    while watch_one_frame(&mut viewer) < from + 120 {}
    assert_eq!(bytes_in(&folder), before, "the session folder grew while the program ran: {:?}", names_in(&folder));
}

#[test]
fn an_unwatched_terminal_answers_screenshot_snapshot_wait_frame_and_output_since_a_mark() {
    let at = AgentTerminal::new();
    let shot = at.work_folder().join("unwatched.png");
    open_redraw(&at, "qa", "--partial");
    let marked = at.run(&["--session", "qa", "--json", "mark", "running"]).success().json()["frame"]
        .as_u64()
        .expect("the frame of the mark");
    let waited = at.run(&["--session", "qa", "--json", "wait", "--frame", &(marked + 30).to_string()]).success().json();
    assert!(waited["frame"].as_u64().is_some_and(|reached| reached >= marked + 30), "{waited}");

    let snapshot = at.run(&["--session", "qa", "--json", "snapshot"]).success().json();
    assert!(snapshot["frame"].as_u64().is_some_and(|shown| shown >= marked + 30), "{snapshot}");
    assert!(snapshot["lines"].as_array().unwrap().iter().any(|line| line.as_str().unwrap().contains("forwarding")));
    at.run(&["--session", "qa", "screenshot", &shot.to_string_lossy()]).success();
    assert!(std::fs::read(&shot).expect("the screenshot").starts_with(b"\x89PNG\r\n\x1a\n"));
    let printed = at.run(&["--session", "qa", "output", "--since", "running"]).success().stdout();
    assert!(printed.contains("forwarding"), "{printed}");
}

#[test]
fn restart_and_close_add_no_file_to_the_session_folder_and_frames_count_on_across_restart() {
    let at = AgentTerminal::new();
    let folder = at.home().join("sessions").join("qa");
    open_printer(&at, "qa", "open");
    assert_eq!(names_in(&folder), ["activity.jsonl"]);
    let before = frame_of(&at, "qa");

    at.run(&["--session", "qa", "restart", "--wait", "line 3"]).success();
    let after = frame_of(&at, "qa");
    assert!(after > before, "frame {after} after the restart, {before} before it");
    assert_eq!(names_in(&folder), ["activity.jsonl"]);

    at.run(&["--session", "qa", "close"]).success();
    assert_eq!(names_in(&folder), ["activity.jsonl"]);
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
