use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use super::{ActivityEntry, ScreenshotEntry, SessionFolder, Typed, export, file_name, folder, write_whole};
use crate::native::render::Cells;
use crate::protocol::{CommandRecord, RowRange, TabId, request};

fn home() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn typed(words: &[&str]) -> Vec<String> {
    words.iter().map(ToString::to_string).collect()
}

fn entry(command: &[&str]) -> ActivityEntry {
    let words = typed(command);
    ActivityEntry::at(
        &Typed { time: SystemTime::UNIX_EPOCH + Duration::from_millis(1500), words: &words },
        Duration::from_millis(7),
    )
}

fn shot(path: &str, frame: u64) -> ScreenshotEntry {
    ScreenshotEntry {
        time: "t".to_string(),
        tab: "t1".to_string(),
        frame,
        path: PathBuf::from(path),
        by: None,
        width: 2,
        height: 3,
        cells: Cells { scale: 1, cols: 4, rows: RowRange { first: 1, last: 2 } },
    }
}

fn commands(entries: &[serde_json::Value]) -> Vec<String> {
    entries.iter().map(|entry| entry["command"].as_str().unwrap().to_string()).collect()
}

#[test]
fn a_session_name_becomes_a_file_name_with_unsafe_bytes_escaped() {
    assert_eq!(
        ["work", "a b/c", ".hidden", "v1.2", "", "é_-"].map(file_name),
        ["work", "a%20b%2Fc", "%2Ehidden", "v1.2", "%", "%C3%A9_-"]
    );
    assert_eq!(folder(Path::new("/h"), "a b"), PathBuf::from("/h/sessions/a%20b"));
}

#[test]
fn a_started_folder_replaces_the_last_one_and_hands_out_tabs_from_one() {
    let home = home();
    let old = folder(home.path(), "work").join("left-over");
    std::fs::create_dir_all(&old).unwrap();
    let started = SessionFolder::start(home.path(), "work").unwrap();
    assert!(!old.exists() && folder(home.path(), "work").is_dir());
    assert_eq!(
        [started.next_tab(), started.next_tab(), started.next_tab()],
        [TabId::FIRST, TabId::FIRST.next(), TabId::FIRST.next().next()]
    );
    std::fs::write(home.path().join("sessions/blocked"), "").unwrap();
    assert!(SessionFolder::start(&home.path().join("sessions/blocked"), "x").is_err());
}

#[test]
fn an_entry_records_its_call_time_and_duration_and_keeps_at_most_64_kib_of_output() {
    let mut logged = entry(&["press", "a b"]);
    assert_eq!(
        (logged.time.as_str(), logged.command.as_str(), logged.duration_ms),
        ("1970-01-01T00:00:01.500Z", "press 'a b'", 7)
    );
    logged.printed("é".repeat(40_000));
    assert!(logged.output.starts_with("éé") && logged.output.ends_with("\n… 14464 more bytes not kept\n"));
    assert_eq!(logged.output.len(), 65_536 + "\n… 14464 more bytes not kept\n".len());
    logged.printed("short".to_string());
    assert_eq!(logged.output, "short");
}

#[test]
fn a_call_is_pending_from_its_start_and_listed_with_its_tab() {
    let home = home();
    let started = Arc::new(SessionFolder::start(home.path(), "work").unwrap());
    let logged = started.logged();
    let words = typed(&["wait", "--idle"]);
    let _pending = started.begin(&Typed { time: SystemTime::UNIX_EPOCH, words: &words }, Some("t1".to_string()));
    let _other = started.begin(&Typed { time: SystemTime::UNIX_EPOCH, words: &words }, None);
    let (entries, end, waiting) = started.activity_and_pending_since(0).unwrap();
    let listed: Vec<_> =
        waiting.iter().map(|call| (call.tab.as_deref(), call.command.as_str(), call.time.as_str())).collect();
    assert_eq!((entries.len(), end, logged.has_changed().unwrap()), (0, 0, true));
    assert_eq!(
        listed,
        [(Some("t1"), "wait --idle", "1970-01-01T00:00:00.000Z"), (None, "wait --idle", "1970-01-01T00:00:00.000Z")]
    );
}

#[test]
fn a_pending_call_ends_when_it_is_logged_or_dropped_and_the_log_reads_on_from_an_offset() {
    let home = home();
    let started = Arc::new(SessionFolder::start(home.path(), "work").unwrap());
    let mut logged = started.logged();
    let words = typed(&["wait", "--idle"]);
    let pending = started.begin(&Typed { time: SystemTime::UNIX_EPOCH, words: &words }, None);
    let other = started.begin(&Typed { time: SystemTime::UNIX_EPOCH, words: &words }, None);
    logged.mark_unchanged();
    pending.log(&entry(&["wait", "--idle"]));
    let after_log = logged.has_changed().unwrap();
    logged.mark_unchanged();
    started.log(&entry(&["snapshot"]));
    logged.mark_unchanged();
    drop(other);
    let (entries, end, waiting) = started.activity_and_pending_since(0).unwrap();
    assert_eq!(
        (after_log, logged.has_changed().unwrap(), commands(&entries), waiting.len()),
        (true, true, vec!["wait --idle".to_string(), "snapshot".to_string()], 0)
    );
    assert_eq!(started.activity_since(end).unwrap(), (Vec::new(), end));
}

#[test]
fn a_half_written_activity_line_is_read_once_it_is_whole() {
    let home = home();
    let started = SessionFolder::start(home.path(), "work").unwrap();
    assert_eq!(started.activity_since(5).unwrap(), (Vec::new(), 5));
    started.log(&entry(&["open", "vim"]));
    let log = folder(home.path(), "work").join("activity.jsonl");
    let whole = std::fs::read(&log).unwrap().len() as u64;
    std::fs::write(&log, [std::fs::read(&log).unwrap(), b"{\"command\":".to_vec()].concat()).unwrap();
    let (entries, end) = started.activity_since(0).unwrap();
    assert_eq!((commands(&entries), end), (vec!["open vim".to_string()], whole));
    std::fs::write(&log, b"not json\n").unwrap();
    assert!(started.activity_since(0).is_err());
}

#[test]
fn a_screenshot_listed_again_replaces_its_earlier_entry() {
    let home = home();
    let started = SessionFolder::start(home.path(), "work").unwrap();
    let listed = started.screenshots_listed();
    let before = started.screenshots().unwrap().len();
    for (path, frame) in [("a.png", 1), ("b.png", 2), ("a.png", 3)] {
        started.keep_screenshot(&shot(path, frame));
    }
    let kept: Vec<(PathBuf, u64)> =
        started.screenshots().unwrap().into_iter().map(|shot| (shot.path, shot.frame)).collect();
    assert_eq!(
        (before, listed.has_changed().unwrap(), kept),
        (0, true, vec![(PathBuf::from("b.png"), 2), (PathBuf::from("a.png"), 3)])
    );
}

#[test]
fn a_half_written_screenshot_line_waits_and_a_broken_one_fails() {
    let home = home();
    let started = SessionFolder::start(home.path(), "work").unwrap();
    started.keep_screenshot(&shot("a.png", 1));
    let list = folder(home.path(), "work").join("screenshots.jsonl");
    std::fs::write(&list, [std::fs::read(&list).unwrap(), b"{\"time\"".to_vec()].concat()).unwrap();
    let whole = started.screenshots().unwrap().len();
    std::fs::write(&list, b"{}\n").unwrap();
    assert_eq!((whole, started.screenshots().is_err()), (1, true));
}

#[test]
fn a_tabs_command_log_gets_a_line_per_command_with_its_output() {
    let home = home();
    let started = SessionFolder::start(home.path(), "work").unwrap();
    let record = CommandRecord {
        id: 3,
        command: "ls".to_string(),
        cwd: "/".to_string(),
        started_at_ms: 1,
        ended_at_ms: Some(2),
        duration_ms: 1,
        exit_code: Some(0),
        first_frame: 1,
        last_frame: Some(2),
    };
    started.command_log("t2").append(&record, "out");
    started.command_log("t2").append(&record, "again");
    let written = std::fs::read_to_string(folder(home.path(), "work").join("t2.commands.jsonl")).unwrap();
    let lines: Vec<serde_json::Value> = written.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(
        (lines.len(), lines[0]["id"].as_u64(), lines[0]["output"].as_str(), lines[1]["output"].as_str()),
        (2, Some(3), Some("out"), Some("again"))
    );
}

#[test]
fn a_whole_write_lands_in_one_step_and_leaves_no_partial_file() {
    let home = home();
    let path = home.path().join("deep/er/file.txt");
    write_whole(&path, b"one").unwrap();
    write_whole(&path, b"two").unwrap();
    let left: Vec<_> =
        std::fs::read_dir(path.parent().unwrap()).unwrap().map(|entry| entry.unwrap().file_name()).collect();
    assert_eq!(
        (std::fs::read_to_string(&path).unwrap(), left),
        ("two".to_string(), vec![std::ffi::OsString::from("file.txt")])
    );
    assert!(write_whole(Path::new("/"), b"x").is_err());
}

fn exported_session(home: &Path) -> (PathBuf, PathBuf) {
    let started = SessionFolder::start(home, "my-work").unwrap();
    let shots = home.join("shots");
    std::fs::create_dir_all(shots.join("other")).unwrap();
    std::fs::write(shots.join("a.png"), "A").unwrap();
    std::fs::write(shots.join("other/a.png"), "B").unwrap();
    let mut call = entry(&["screenshot"]);
    call.screenshots = vec![shots.join("a.png"), shots.join("gone.png")];
    started.log(&call);
    started.keep_screenshot(&shot(&shots.join("other/a.png").to_string_lossy(), 1));
    started.keep_screenshot(&shot(&shots.join("a.png").to_string_lossy(), 2));
    let session = folder(home, "my-work");
    std::fs::create_dir_all(session.join("sub")).unwrap();
    std::fs::write(session.join("sub/notes.txt"), "kept\npartial").unwrap();
    std::fs::write(session.join("t1.commands.jsonl"), "{}\nhalf").unwrap();
    (shots, home.join("out/trace.zip"))
}

#[test]
fn an_export_zips_the_folder_with_whole_lines_and_each_screenshot_once() {
    let home = home();
    let (shots, to) = exported_session(home.path());
    let exported =
        export(home.path(), &request::TraceExport { session: "my-work".parse().unwrap(), path: to.clone() }).unwrap();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&to).unwrap()).unwrap();
    let mut read = |name: &str| {
        let mut text = String::new();
        zip.by_name(name).unwrap().read_to_string(&mut text).unwrap();
        text
    };
    let files = [
        read("my-work/sub/notes.txt"),
        read("my-work/t1.commands.jsonl"),
        read("my-work/screenshots/a.png"),
        read("my-work/screenshots/2-a.png"),
    ];
    assert_eq!(files, ["kept\npartial", "{}\n", "A", "B"]);
    assert_eq!(
        (exported.files, exported.missing, exported.bytes),
        (6, vec![shots.join("gone.png")], std::fs::metadata(&to).unwrap().len())
    );
}

#[test]
fn a_session_with_no_folder_has_nothing_to_export() {
    let home = home();
    let missing = export(
        home.path(),
        &request::TraceExport { session: "never".parse().unwrap(), path: home.path().join("t.zip") },
    );
    assert_eq!(
        missing.err().map(|e| e.to_string()),
        Some(
            "session never has nothing to export; open a terminal in it first with `agent-terminal open <program>`"
                .to_string()
        )
    );
}
