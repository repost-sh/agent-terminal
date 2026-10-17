use std::path::Path;
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

use crate::support::{AgentTerminal, DashboardSocket, Redraw, dashboard, fixture, open_redraw};

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

fn open_printer(at: &AgentTerminal, session: &str, tab: &str) {
    at.run(&["--session", session, "open", "--name", tab, "--wait", "line 3", &fixture("line-printer"), "--hold"])
        .success();
}

#[test]
fn no_tab_of_a_session_leaves_a_file_of_its_own_in_the_session_folder() {
    let at = AgentTerminal::new();
    let folder = at.home().join("sessions").join("qa");
    at.run(&["session", "new", "qa"]).success();
    assert_eq!(names_in(&folder), Vec::<String>::new());
    open_printer(&at, "qa", "app");
    at.run(&["--session", "qa", "tab", "new", "--name", "api", "--wait", "line 3", &fixture("line-printer"), "--hold"])
        .success();
    at.run(&["--session", "qa", "tab", "close", "app"]).success();
    at.run(&["--session", "qa", "tab", "new", "--name", "web", "--wait", "line 3", &fixture("line-printer"), "--hold"])
        .success();
    assert_eq!(names_in(&folder), ["activity.jsonl"]);
}

#[test]
fn a_session_that_starts_again_starts_a_fresh_folder() {
    let at = AgentTerminal::new();
    let folder = at.home().join("sessions").join("qa");
    let shot = at.work_folder().join("first-run.png").to_string_lossy().into_owned();
    at.run(&["session", "new", "qa"]).success();
    open_printer(&at, "qa", "app");
    at.run(&["--session", "qa", "screenshot", &shot]).success();
    at.run(&["--session", "qa", "close", "--all"]).success();
    assert_eq!(names_in(&folder), ["activity.jsonl", "screenshots.jsonl"]);

    at.run(&["session", "new", "qa"]).success();
    open_printer(&at, "qa", "app");
    assert_eq!(names_in(&folder), ["activity.jsonl"]);
    assert_eq!(at.activity("qa").len(), 1);
}

#[test]
fn simultaneous_session_news_of_one_name_make_one_session_whose_opens_share_one_folder() {
    let at = AgentTerminal::new();
    let created: Vec<i32> = std::thread::scope(|scope| {
        let news: Vec<_> = (0..4).map(|_| scope.spawn(|| at.run(&["session", "new", "qa"]).code())).collect();
        news.into_iter().map(|new| new.join().unwrap()).collect()
    });
    assert_eq!((created.iter().filter(|code| **code == 0).count(), created.len()), (1, 4), "{created:?}");
    let at = &at;
    std::thread::scope(|scope| {
        for tab in ["app", "api", "web", "db"] {
            scope.spawn(move || open_printer(at, "qa", tab));
        }
    });
    assert_eq!(names_in(&at.home().join("sessions").join("qa")), ["activity.jsonl"]);
    assert_eq!(at.activity("qa").len(), 4);
}

#[test]
fn a_session_name_that_is_empty_or_a_path_is_refused_and_makes_no_folder() {
    let at = AgentTerminal::new();
    let calls = [
        &["session", "new", "../escape/qa"][..],
        &["session", "new", ""],
        &["--session", "", "open", "--name", "app", "cat"],
    ];
    let refused: Vec<(i32, bool)> = calls
        .iter()
        .map(|args| at.run(args))
        .map(|run| (run.code(), run.stderr().contains("is not a name: a name is 1 to 24 characters")))
        .collect();
    assert_eq!(refused, [(1, true), (1, true), (1, true)]);
    assert_eq!(names_in(&at.home().join("sessions")), ["work"]);
    assert!(!at.home().join("escape").exists());
}

#[test]
fn a_program_repainting_fast_adds_nothing_to_the_session_folder() {
    let at = AgentTerminal::new();
    let folder = at.home().join("sessions").join("qa");
    let shot = at.work_folder().join("fast.png").to_string_lossy().into_owned();
    at.run(&["session", "new", "qa"]).success();
    open_redraw(&at, &Redraw { session: "qa", hz: "60", repaint: "--full" });
    let opened = at.frame("qa");
    at.run(&["--session", "qa", "wait", "--frame", &(opened + 120).to_string()]).success();
    at.run(&["--session", "qa", "screenshot", &shot]).success();
    assert_eq!(names_in(&folder), ["activity.jsonl", "screenshots.jsonl"]);

    let port = dashboard(&at);
    let mut viewer = DashboardSocket::open(port, "/api/session/qa/app/stream");
    let before = bytes_in(&folder);
    let from = viewer.acknowledge_next();
    while viewer.acknowledge_next() < from + 120 {}
    assert_eq!(bytes_in(&folder), before, "the session folder grew while the program ran: {:?}", names_in(&folder));
}

#[test]
fn an_unwatched_terminal_answers_screenshot_snapshot_wait_frame_and_output_since_a_mark() {
    let at = AgentTerminal::new();
    let shot = at.work_folder().join("unwatched.png");
    at.run(&["session", "new", "qa"]).success();
    open_redraw(&at, &Redraw { session: "qa", hz: "60", repaint: "--partial" });
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
fn restart_and_close_add_no_file_to_the_session_folder() {
    let at = AgentTerminal::new();
    let folder = at.home().join("sessions").join("qa");
    at.run(&["session", "new", "qa"]).success();
    open_printer(&at, "qa", "app");
    assert_eq!(names_in(&folder), ["activity.jsonl"]);

    at.run(&["--session", "qa", "restart", "--wait", "line 3"]).success();
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
    at.run(&["open", "--name", "app", "--size", "20x4", "--wait", "line 3", &printer, "--hold"]).success();
    at.run(&["wait", "--cursor", "4,1"]).success();
    at.run(&["screenshot", &first]).success();
    let first_frame = at.frame("work");
    at.run(&["type", "x", "--wait", "x", "--screenshot"]).success();
    let typed_frame = at.frame("work");
    at.run(&["screenshot", "--scale", "2", "--rows", "2:3", &crop]).success();
    at.run(&["screenshot", &text]).success();
    assert_eq!(at.run(&["diff", "screenshot", "--baseline", &first, "--output", &diff]).code(), 2);

    let mut listed = screenshots_listed(&at, "work");
    for entry in &mut listed {
        assert_recent_utc_millis(entry, began);
        entry.as_object_mut().unwrap().remove("time");
    }
    let typed = at.home().join("screenshots").join(format!("work-app-{typed_frame}.png"));
    let full = json!({ "first": 1, "last": 4 });
    assert_eq!(
        listed,
        [
            json!({ "tab": "app", "frame": first_frame, "path": first, "by": format!("screenshot {first}"),
                "width": 200, "height": 100, "scale": 1, "cols": 20, "rows": full }),
            json!({ "tab": "app", "frame": typed_frame, "path": typed.to_string_lossy(), "by": "type x --wait x --screenshot",
                "width": 200, "height": 100, "scale": 1, "cols": 20, "rows": full }),
            json!({ "tab": "app", "frame": typed_frame, "path": crop, "by": format!("screenshot --scale 2 --rows 2:3 {crop}"),
                "width": 400, "height": 120, "scale": 2, "cols": 20, "rows": { "first": 2, "last": 3 } }),
        ]
    );
}

#[test]
fn a_session_whose_folder_cannot_be_made_opens_nothing_and_the_daemon_keeps_serving() {
    let at = AgentTerminal::new();
    let sessions = at.home().join("sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::write(sessions.join("blocked"), "a file where the session's folder goes").unwrap();

    let refused = at.run(&["session", "new", "blocked"]);
    assert_eq!(refused.code(), 1, "{}", refused.stderr());
    assert!(
        refused
            .stderr()
            .starts_with(&format!("error: clear the previous session folder {}: ", sessions.join("blocked").display())),
        "{}",
        refused.stderr()
    );
    assert_eq!(at.run(&["session", "list"]).success().stdout(), "work · no tabs\n");
    at.run(&["session", "new", "open"]).success();
    at.run(&["--session", "open", "open", "--name", "app", "--size", "20x2", "cat"]).success();
}
