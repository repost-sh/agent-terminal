use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use crate::support::{AgentTerminal, fixture};

fn unzip(path: &Path) -> BTreeMap<String, String> {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
    let mut archive = zip::ZipArchive::new(file).expect("a zip file");
    (0..archive.len())
        .map(|i| {
            let mut entry = archive.by_index(i).unwrap();
            let mut contents = String::new();
            entry.read_to_string(&mut contents).unwrap();
            (entry.name().unwrap().into_owned(), contents)
        })
        .collect()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn open_printer(at: &AgentTerminal, session: &str, command: &[&str]) {
    let printer = fixture("line-printer");
    let mut args = vec!["--session", session];
    args.extend_from_slice(command);
    args.extend_from_slice(&["--wait", "line 3", &printer, "--hold"]);
    at.run(&args).success();
}

#[test]
fn trace_export_zips_the_session_folder() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    let folder = at.home().join("sessions").join("qa");
    open_printer(&at, "qa", &["open"]);
    open_printer(&at, "qa", &["tab", "new"]);
    let activity = read(&folder.join("activity.jsonl"));

    let exported = at.run_in(&work, &["--session", "qa", "trace", "export", "evidence/qa.zip"]).success().stdout();
    let zip = work.join("evidence").join("qa.zip");
    let kb = std::fs::metadata(&zip).unwrap().len().div_ceil(1024);
    assert_eq!(exported, format!("exported session qa to {} (1 file, {kb} KB)\n", zip.display()));
    let files = unzip(&zip);
    assert_eq!(files.keys().collect::<Vec<_>>(), ["qa/activity.jsonl"]);
    assert_eq!(files["qa/activity.jsonl"], activity);

    let last = read(&folder.join("activity.jsonl")).lines().last().unwrap().to_string();
    let last: serde_json::Value = serde_json::from_str(&last).unwrap();
    assert_eq!(last["command"], "--session qa trace export evidence/qa.zip");
}

fn unzip_bytes(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
    let mut archive = zip::ZipArchive::new(file).expect("a zip file");
    (0..archive.len())
        .map(|i| {
            let mut entry = archive.by_index(i).unwrap();
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents).unwrap();
            (entry.name().unwrap().into_owned(), contents)
        })
        .collect()
}

#[test]
fn trace_export_adds_the_screenshots_the_session_saved() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    open_printer(&at, "default", &["open"]);
    let frame = at.run(&["get", "frame"]).success().stdout().trim().to_string();
    at.run_in(&work, &["--screenshot-dir", "shots", "screenshot"]).success();
    at.run_in(&work, &["screenshot", "named.png"]).success();
    at.run_in(&work, &["diff", "screenshot", "--baseline", "named.png", "--output", "changes.png"]).success();
    at.run_in(&work, &["screenshot", "gone.png"]).success();
    std::fs::remove_file(work.join("gone.png")).expect("remove a screenshot");

    let exported = at.run_in(&work, &["trace", "export", "trace.zip"]).success().stdout();
    let files = unzip_bytes(&work.join("trace.zip"));
    let default_shot = format!("default-t1-{frame}.png");
    let saved = [
        (format!("default/screenshots/{default_shot}"), work.join("shots").join(&default_shot)),
        ("default/screenshots/named.png".to_string(), work.join("named.png")),
        ("default/screenshots/changes.png".to_string(), work.join("changes.png")),
    ];
    for (name, path) in &saved {
        let kept = std::fs::read(path).expect("read a saved file");
        assert!(files.get(name) == Some(&kept), "{name} is not in the zip as {} was saved", path.display());
    }
    let folder_files = ["default/activity.jsonl", "default/screenshots.jsonl"];
    assert!(folder_files.iter().all(|name| files.contains_key(*name)), "{:?}", files.keys().collect::<Vec<_>>());
    assert_eq!(files.len(), folder_files.len() + saved.len(), "{:?}", files.keys().collect::<Vec<_>>());
    let gone = work.join("gone.png");
    assert!(
        exported.ends_with(&format!("1 file the session saved is gone and is not in it: {}\n", gone.display())),
        "{exported}"
    );
}

#[test]
fn trace_export_includes_the_command_log_of_a_shell_tab() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    let folder = at.home().join("sessions").join("default");
    let cwd = work.to_string_lossy().into_owned();
    at.run_with_env(&["shell", "--shell", "bash", "--cwd", &cwd], &[("HOME", work.as_path())]).success();
    at.run(&["exec", "echo traced"]).success();

    at.run_in(&work, &["trace", "export", "trace.zip"]).success();
    let files = unzip(&work.join("trace.zip"));
    assert_eq!(files.keys().collect::<Vec<_>>(), ["default/activity.jsonl", "default/t1.commands.jsonl"]);
    assert_eq!(files["default/t1.commands.jsonl"], read(&folder.join("t1.commands.jsonl")));
    the_command_log_holds_the_command_its_exit_and_output(&files["default/t1.commands.jsonl"]);
}

fn the_command_log_holds_the_command_its_exit_and_output(log: &str) {
    let logged: Vec<serde_json::Value> = log.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(logged.len(), 1, "{logged:?}");
    assert_eq!(logged[0]["command"], "echo traced");
    assert_eq!(logged[0]["exit_code"], 0);
    assert_eq!(logged[0]["output"], "traced\n");
}

#[test]
fn trace_export_answers_in_json() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    open_printer(&at, "default", &["open"]);
    let exported = at.run_in(&work, &["--json", "trace", "export", "trace.zip"]).success().json();
    let zip = work.join("trace.zip");
    assert_eq!(
        exported,
        serde_json::json!({
            "session": "default",
            "path": zip.to_string_lossy(),
            "files": 1,
            "bytes": std::fs::metadata(&zip).unwrap().len(),
        })
    );
}

#[test]
fn trace_export_still_works_after_the_session_closed() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    open_printer(&at, "default", &["open"]);
    at.run(&["close"]).success();
    at.run_in(&work, &["trace", "export", "after-close.zip"]).success();
    let files = unzip(&work.join("after-close.zip"));
    assert_eq!(files.keys().collect::<Vec<_>>(), ["default/activity.jsonl"]);
    assert!(files["default/activity.jsonl"].lines().last().unwrap().contains(r#""command":"close""#));
}

#[test]
fn trace_export_of_a_session_that_never_ran_explains_what_to_do() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    let exported = at.run_in(&work, &["--session", "ghost", "trace", "export", "ghost.zip"]);
    assert_eq!(exported.code(), 1);
    assert_eq!(
        exported.stderr(),
        "error: session ghost has nothing to export; open a terminal in it first with `agent-terminal open <program>`\n"
    );
    assert!(!work.join("ghost.zip").exists());
}

#[test]
fn trace_export_leaves_out_an_event_still_being_written() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    open_printer(&at, "default", &["open"]);
    at.run_in(&work, &["screenshot", "listed.png"]).success();
    let folder = at.home().join("sessions/default");
    let written = |name: &str| read(&folder.join(name));
    let (activity, screenshots) = (written("activity.jsonl"), written("screenshots.jsonl"));
    let half_written = [
        ("activity.jsonl", r#"{"time":"2026-10-14T14:01:22.051Z","tab":"t1","comm"#),
        ("screenshots.jsonl", r#"{"time":"2026-10-14T14:01:22.060Z","tab":"t1","fra"#),
    ];
    for (name, half) in half_written {
        let mut file = std::fs::OpenOptions::new().append(true).open(folder.join(name)).expect("open a session file");
        std::io::Write::write_all(&mut file, half.as_bytes()).expect("start an event without finishing it");
    }

    at.run_in(&work, &["trace", "export", "writing.zip"]).success();
    let files = unzip_bytes(&work.join("writing.zip"));
    assert_eq!(files["default/activity.jsonl"], activity.as_bytes());
    assert_eq!(files["default/screenshots.jsonl"], screenshots.as_bytes());
}

#[test]
fn trace_export_says_it_zips_the_activity_log_the_screenshot_list_and_the_command_logs() {
    let at = AgentTerminal::new();
    let help = at.run(&["trace", "--help"]).success().stdout();
    assert!(
        help.contains("Zip the session folder (activity log, screenshot list, command logs) with its screenshots"),
        "{help}"
    );
}
