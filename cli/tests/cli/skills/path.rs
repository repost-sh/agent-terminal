use std::path::Path;

use super::shipped::shipped;
use crate::support::{AgentTerminal, VERSION};

#[test]
fn skills_path_points_at_a_skills_files() {
    let at = AgentTerminal::new();
    let printed = at.run(&["skills", "path", "tui-qa"]).success().stdout();
    let dir = at.home().join("skills").join(VERSION).join("tui-qa");
    assert_eq!(printed, format!("{}\n", dir.display()));
    let guide = at.run(&["skills", "get", "tui-qa"]).success().stdout();
    assert_eq!(std::fs::read_to_string(dir.join("SKILL.md")).unwrap(), guide);
    assert_eq!(
        std::fs::read_to_string(dir.join("templates/report.md")).unwrap(),
        shipped("skill-data/tui-qa/templates/report.md")
    );
}

#[test]
fn skills_path_without_a_name_is_the_folder_of_every_skill() {
    let at = AgentTerminal::new();
    let printed = at.run(&["skills", "path"]).success().stdout();
    let dir = at.home().join("skills").join(VERSION);
    assert_eq!(printed, format!("{}\n", dir.display()));
    let mut names: Vec<String> =
        std::fs::read_dir(&dir).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["agent-terminal", "cli-qa", "core", "tui-qa"]);
    let guide = at.run(&["skills", "get", "core"]).success().stdout();
    assert_eq!(std::fs::read_to_string(dir.join("core/SKILL.md")).unwrap(), guide);
}

#[test]
fn skills_path_as_json_and_for_an_unknown_skill() {
    let at = AgentTerminal::new();
    let json = at.run(&["--json", "skills", "path", "core"]).success().json();
    let dir = at.home().join("skills").join(VERSION).join("core");
    assert_eq!(json, serde_json::json!({ "path": dir }));
    let run = at.run(&["skills", "path", "nope"]);
    assert_eq!(run.code(), 1);
    assert_eq!(run.stderr(), "error: no skill named nope; list them with `agent-terminal skills list`\n");
}

#[test]
fn simultaneous_skills_path_calls_each_see_complete_files() {
    let at = AgentTerminal::new();
    let template = shipped("skill-data/tui-qa/templates/report.md");
    std::thread::scope(|scope| {
        let readers: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    let printed = at.run(&["skills", "path", "tui-qa"]).success().stdout();
                    std::fs::read_to_string(Path::new(printed.trim_end()).join("templates/report.md")).unwrap()
                })
            })
            .collect();
        for reader in readers {
            assert_eq!(reader.join().unwrap(), template);
        }
    });
}

#[test]
fn skills_path_removes_the_folders_older_versions_left() {
    let at = AgentTerminal::new();
    let cache = at.home().join("skills");
    for left in ["0.0.1", "0.1.0-alpha.1", "999.0.0", "notes"] {
        std::fs::create_dir_all(cache.join(left).join("tui-qa/templates")).unwrap();
        std::fs::write(cache.join(left).join("tui-qa/templates/report.md"), "left behind\n").unwrap();
    }
    std::thread::scope(|scope| {
        let calls: Vec<_> = (0..8).map(|_| scope.spawn(|| at.run(&["skills", "path"]).code())).collect();
        let codes: Vec<i32> = calls.into_iter().map(|call| call.join().unwrap()).collect();
        assert_eq!(codes, vec![0; 8]);
    });
    let left: std::collections::BTreeSet<String> =
        std::fs::read_dir(&cache).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    assert_eq!(left, [VERSION, "999.0.0", "notes"].map(String::from).into());
    assert_eq!(
        std::fs::read_to_string(cache.join("999.0.0/tui-qa/templates/report.md")).unwrap(),
        "left behind\n",
        "a newer version's folder is not this version's to remove"
    );
}

#[test]
fn skills_path_says_which_older_folder_it_could_not_remove() {
    use std::os::unix::fs::PermissionsExt;
    let at = AgentTerminal::new();
    let older = at.home().join("skills").join("0.0.1");
    std::fs::create_dir_all(older.join("tui-qa")).unwrap();
    let mut locked = std::fs::metadata(&older).unwrap().permissions();
    locked.set_mode(0o500);
    std::fs::set_permissions(&older, locked.clone()).unwrap();
    let refused = at.run(&["skills", "path"]);
    locked.set_mode(0o700);
    std::fs::set_permissions(&older, locked).unwrap();
    assert_eq!(refused.code(), 1, "{}", refused.stderr());
    assert!(refused.stderr().starts_with(&format!("error: remove {}: ", older.display())), "{}", refused.stderr());
}
