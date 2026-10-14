use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::support::{AgentTerminal, fixture};

fn recording(at: &AgentTerminal, session: &str, tab: &str) -> PathBuf {
    at.home().join("sessions").join(session).join(format!("{tab}.cast"))
}

fn replay(at: &AgentTerminal, cast: &Path, size: &str) -> Value {
    let cast = cast.to_string_lossy();
    let replayed = at
        .run(&[
            "--json",
            "--session",
            "replay",
            "open",
            "--size",
            size,
            "--wait",
            "the replay never shows this",
            &fixture("cast-replay"),
            &cast,
        ])
        .success()
        .json();
    assert_eq!(replayed["exit_code"], 0, "the replay did not finish cleanly: {replayed}");
    replayed
}

#[track_caller]
fn assert_same_screen(replayed: &Value, original: &Value) {
    for field in ["lines", "cursor", "cursor_visible", "alt_screen", "cols", "rows"] {
        assert_eq!(replayed[field], original[field], "{field} differs\nreplayed: {replayed}\noriginal: {original}");
    }
}

#[test]
fn a_tab_recording_replays_to_the_same_final_screen() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "60x12", "--wait", "Selected: alpha.txt", &fixture("fixture-tui")]).success();
    at.run(&["press", "j", "--wait", "Selected: beta.txt"]).success();
    at.run(&["press", "j", "--wait", "Selected: gamma.txt"]).success();
    let original = at.run(&["--json", "snapshot"]).success().json();

    let replayed = replay(&at, &recording(&at, "default", "t1"), "60x12");
    assert_same_screen(&replayed, &original);
    assert!(replayed["lines"][1].as_str().unwrap().contains("Selected: gamma.txt"), "{replayed}");
}

fn events(cast: &Path) -> (Value, Vec<(f64, String, String)>) {
    let text = std::fs::read_to_string(cast).unwrap_or_else(|e| panic!("read {}: {e}", cast.display()));
    let mut lines = text.lines();
    let header = serde_json::from_str(lines.next().expect("a header line")).expect("the header is JSON");
    let events = lines.map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}"))).collect();
    (header, events)
}

fn of_kind(events: &[(f64, String, String)], kind: &str) -> Vec<String> {
    events.iter().filter(|(_, k, _)| k == kind).map(|(_, _, data)| data.clone()).collect()
}

#[test]
fn the_recording_has_the_input_sent_and_every_resize() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "40x6", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    at.run(&["type", "hé", "--wait", "hé"]).success();
    at.run(&["resize", "50x8", "--wait", "hé"]).success();

    let (header, events) = events(&recording(&at, "default", "t1"));
    assert_eq!(header["version"], 2);
    assert_eq!((header["width"].clone(), header["height"].clone()), (40.into(), 6.into()));
    assert!(header["command"].as_str().unwrap().ends_with("line-printer --hold"), "{header}");
    assert_eq!(of_kind(&events, "i"), ["h", "é"]);
    assert_eq!(of_kind(&events, "r"), ["50x8"]);
    assert_eq!(of_kind(&events, "o").concat(), "line 1\r\nline 2\r\nline 3\r\nhé");
    assert!(events.windows(2).all(|pair| pair[0].0 <= pair[1].0), "event times go backwards: {events:?}");
}

#[test]
fn the_recording_keeps_the_history_the_scrollback_dropped() {
    let at = AgentTerminal::new();
    at.run(&["open", "--wait", "line 2500", &fixture("line-printer"), "--lines", "2500", "--hold"]).success();
    let snapshot = at.run(&["--json", "snapshot", "--scrollback"]).success().json();
    let scrollback = snapshot["scrollback"].as_array().unwrap();
    assert_eq!(scrollback.len(), 2000);
    assert_eq!(scrollback[0], "line 462");

    let (_, events) = events(&recording(&at, "default", "t1"));
    let printed = of_kind(&events, "o").concat();
    assert!(printed.starts_with("line 1\r\nline 2\r\nline 3\r\n"), "{}", &printed[..40]);
    assert!(printed.ends_with("line 2499\r\nline 2500\r\n"));
}

#[test]
fn a_restart_continues_the_tab_recording_from_a_fresh_screen() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "40x6", "--wait", "line 2", &fixture("line-printer"), "--lines", "2", "--hold"])
        .success();
    at.run(&["restart", "--wait", "line 2"]).success();
    let original = at.run(&["--json", "snapshot"]).success().json();
    assert_eq!(original["lines"], serde_json::json!(["line 1", "line 2", "", "", "", ""]));

    let replayed = replay(&at, &recording(&at, "default", "t1"), "40x6");
    assert_same_screen(&replayed, &original);
}
