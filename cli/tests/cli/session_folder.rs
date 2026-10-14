use std::path::Path;

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
