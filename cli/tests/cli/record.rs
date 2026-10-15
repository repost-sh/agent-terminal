use std::path::Path;

use serde_json::Value;

use crate::support::{AgentTerminal, fixture};

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

fn replay(at: &AgentTerminal, cast: &Path, size: &str) -> Value {
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
            &cast.to_string_lossy(),
        ])
        .success()
        .json();
    assert_eq!(replayed["exit_code"], 0, "the replay did not finish cleanly: {replayed}");
    replayed
}

#[test]
fn record_start_and_stop_save_the_tab_from_the_screen_it_had_at_start() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    at.run_in(&work, &["open", "--size", "60x12", "--wait", "Selected: alpha.txt", &fixture("fixture-tui")]).success();
    at.run(&["press", "j", "--wait", "Selected: beta.txt"]).success();
    let saved = work.join("evidence").join("run.cast");

    let started = at.run_in(&work, &["record", "start", "./evidence/run.cast"]).success();
    assert_eq!(started.stdout(), format!("recording default t1 to {}\n", saved.display()));
    at.run(&["press", "j", "--wait", "Selected: gamma.txt"]).success();
    let original = at.run(&["--json", "snapshot"]).success().json();
    let stopped = at.run(&["--json", "record", "stop"]).success().json();
    assert_eq!(stopped["path"], saved.to_string_lossy().as_ref());
    assert_eq!((stopped["session"].clone(), stopped["tab"].clone()), ("default".into(), "t1".into()));
    assert_eq!(stopped["bytes"], std::fs::metadata(&saved).unwrap().len());
    assert!(stopped["duration_ms"].is_u64(), "{stopped}");

    let (header, events) = events(&saved);
    assert_eq!(header["version"], 2);
    assert_eq!((header["width"].clone(), header["height"].clone()), (60.into(), 12.into()));
    assert_eq!(of_kind(&events, "i"), ["j"]);
    let replayed = replay(&at, &saved, "60x12");
    for field in ["lines", "cursor", "cursor_visible", "alt_screen"] {
        assert_eq!(replayed[field], original[field], "{field} differs\nreplayed: {replayed}\noriginal: {original}");
    }
}

#[test]
fn record_stop_prints_the_file_its_length_and_its_size() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    at.run(&["open", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    at.run_in(&work, &["record", "start", "run.cast"]).success();
    let stopped = at.run(&["record", "stop"]).success().stdout();
    let saved = work.join("run.cast");
    let format =
        format!(r"^saved {} \((\d+ ms|\d+\.\d s|\d+ min \d\d s), 1 KB\)\n$", regex::escape(&saved.to_string_lossy()));
    assert!(regex::Regex::new(&format).unwrap().is_match(&stopped), "{stopped}");
}

#[test]
fn a_saved_recording_starts_at_the_tab_size_and_carries_its_resizes() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    at.run(&["open", "--size", "40x6", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    at.run(&["resize", "50x8", "--wait", "line 3"]).success();
    at.run_in(&work, &["record", "start", "run.cast"]).success();
    at.run(&["resize", "60x10", "--wait", "line 3"]).success();
    at.run(&["type", "x", "--wait", "x"]).success();
    at.run(&["record", "stop"]).success();

    let (header, events) = events(&work.join("run.cast"));
    assert_eq!((header["width"].clone(), header["height"].clone()), (40.into(), 6.into()));
    assert_eq!(of_kind(&events, "r"), ["50x8", "60x10"]);
    assert_eq!(of_kind(&events, "o").concat(), "line 1\r\nline 2\r\nline 3\r\nx");
    assert_eq!(of_kind(&events, "i"), ["x"]);
}

#[test]
fn record_start_without_a_path_saves_into_the_recordings_folder() {
    let at = AgentTerminal::new();
    at.run(&["--session", "qa", "open", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    let frame = at.run(&["--session", "qa", "get", "frame"]).success().stdout().trim().to_string();
    let started = at.run(&["--session", "qa", "--json", "record", "start"]).success().json();
    let saved = at.home().join("recordings").join(format!("qa-t1-{frame}.cast"));
    assert_eq!(started["path"], saved.to_string_lossy().as_ref());
    at.run(&["--session", "qa", "record", "stop"]).success();
    assert!(saved.is_file());
}

#[test]
fn record_start_twice_names_the_file_already_in_use() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    at.run(&["open", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    at.run_in(&work, &["record", "start", "first.cast"]).success();
    let again = at.run_in(&work, &["record", "start", "second.cast"]);
    assert_eq!(again.code(), 1);
    assert_eq!(
        again.stderr(),
        format!(
            "error: default t1 is already being recorded to {}; stop with `agent-terminal record stop`\n",
            work.join("first.cast").display()
        )
    );
    assert!(!work.join("second.cast").exists());
}

#[test]
fn record_stop_without_a_recording_explains_how_to_start_one() {
    let at = AgentTerminal::new();
    at.run(&["open", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    let stopped = at.run(&["record", "stop"]);
    assert_eq!(stopped.code(), 1);
    assert_eq!(
        stopped.stderr(),
        "error: default t1 is not being recorded to a file; start with `agent-terminal record start [path]`\n"
    );
}
