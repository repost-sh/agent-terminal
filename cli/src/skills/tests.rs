use std::path::Path;

use clap::Command;

use super::{
    SKILLS, Stub, find, get, install_stub_at, installed_stub_at, list, path_in, repair_stub_at, stamped_version,
};
use crate::version::VERSION;

fn stub_of_version(version: &str) -> String {
    format!("---\nname: agent-terminal\ndescription: x\nmetadata:\n  version: \"{version}\"\n---\nbody\n")
}

fn stub(home: &Path) -> std::path::PathBuf {
    home.join(".claude/skills/agent-terminal/SKILL.md")
}

#[test]
fn the_list_names_each_skill_but_the_stub_with_the_first_sentence_of_its_description() {
    let text = list(false).unwrap();
    let names: Vec<&str> = text.lines().map(|line| line.split_whitespace().next().unwrap()).collect();
    assert_eq!(names, ["core", "tui-qa", "cli-qa"]);
    assert!(
        text.lines().all(|line| line.get(6..8) == Some("  ")
            && !line[8..].starts_with(' ')
            && line.ends_with('.')
            && !line.contains(". ")),
        "{text}"
    );
    let json: serde_json::Value = serde_json::from_str(&list(true).unwrap()).unwrap();
    let described: Vec<_> =
        json["skills"].as_array().unwrap().iter().map(|skill| skill["name"].as_str().unwrap()).collect();
    assert_eq!(described, ["core", "tui-qa", "cli-qa"]);
    assert!(json["skills"][0]["description"].as_str().unwrap().len() > text.lines().next().unwrap().len() - 8);
}

#[test]
fn a_guide_is_stamped_with_this_version_and_its_full_form_adds_its_files_and_the_reference() {
    let core = get("core", false, false, Command::new("agent-terminal")).unwrap();
    assert!(
        core.starts_with("---\nname: core\n") && core.contains(&format!("metadata:\n  version: \"{VERSION}\"\n---\n")),
        "{core}"
    );
    let full =
        get("core", true, false, Command::new("agent-terminal").subcommand(Command::new("open").about("Run it")))
            .unwrap();
    assert!(
        full.starts_with(&core) && full.contains("\n--- command reference ---\n\n") && full.contains("Run it"),
        "{full}"
    );
    let qa = get("tui-qa", true, false, Command::new("agent-terminal")).unwrap();
    assert!(qa.contains("\n--- templates/report.md ---\n\n") && !qa.contains("command reference"));
    let json: serde_json::Value =
        serde_json::from_str(&get("cli-qa", false, true, Command::new("x")).unwrap()).unwrap();
    assert_eq!(json["name"], "cli-qa");
    assert_eq!(
        get("nope", false, false, Command::new("x")).unwrap_err().to_string(),
        "no skill named nope; list them with `agent-terminal skills list`"
    );
}

#[test]
fn every_built_in_skill_has_frontmatter_a_description_and_its_files() {
    for skill in SKILLS {
        let files = skill.files().unwrap();
        assert_eq!((files[0].0, files.len()), ("SKILL.md", 1 + skill.extras.len()));
        assert!(skill.summary().unwrap().len() <= skill.description().unwrap().len());
    }
    assert!(find("agent-terminal").unwrap().is_stub() && !find("core").unwrap().is_stub());
}

#[test]
fn a_stubs_version_is_read_from_its_metadata() {
    assert_eq!(stamped_version(&stub_of_version("1.2.3")).map(|v| v.to_string()), Some("1.2.3".to_string()));
    assert_eq!(
        stamped_version("---\nmetadata:\n  other: 1\n  version: 2.0.0\n---\n").map(|v| v.to_string()),
        Some("2.0.0".to_string())
    );
    let unversioned = [
        "no frontmatter",
        "---\nname: x\n---\n",
        "---\nmetadata:\n  version: \"x\"\n---\n",
        "---\nmetadata:\nversion: 1.0.0\n---\n",
    ];
    assert!(unversioned.iter().all(|text| stamped_version(text).is_none()));
}

#[test]
fn the_installed_stub_is_missing_current_older_newer_or_unversioned() {
    let home = tempfile::tempdir().unwrap();
    let kind = |contents: Option<&str>| {
        if let Some(contents) = contents {
            std::fs::create_dir_all(stub(home.path()).parent().unwrap()).unwrap();
            std::fs::write(stub(home.path()), contents).unwrap();
        }
        match installed_stub_at(stub(home.path())).unwrap().stub {
            Stub::Missing => "missing".to_string(),
            Stub::Current => "current".to_string(),
            Stub::Older(version) => format!("older {version}"),
            Stub::Newer(version) => format!("newer {version}"),
            Stub::Unversioned => "unversioned".to_string(),
        }
    };
    assert_eq!(
        [
            kind(None),
            kind(Some(&stub_of_version(VERSION))),
            kind(Some(&stub_of_version("0.0.1"))),
            kind(Some(&stub_of_version("99.0.0"))),
            kind(Some("plain"))
        ],
        ["missing", "current", "older 0.0.1", "newer 99.0.0", "unversioned"]
    );
}

#[test]
fn installing_writes_this_versions_stub_unless_a_newer_one_is_there() {
    let home = tempfile::tempdir().unwrap();
    let fresh = install_stub_at(stub(home.path())).unwrap();
    assert_eq!((fresh.installed, fresh.version.as_str()), (true, VERSION));
    assert_eq!(
        stamped_version(&std::fs::read_to_string(stub(home.path())).unwrap()).map(|v| v.to_string()),
        Some(VERSION.to_string())
    );
    std::fs::write(stub(home.path()), stub_of_version("99.0.0")).unwrap();
    let kept = install_stub_at(stub(home.path())).unwrap();
    assert_eq!((kept.installed, kept.version.as_str()), (false, "99.0.0"));
    std::fs::write(stub(home.path()), format!("{}{}", stub_of_version("0.0.1"), "x".repeat(10_000))).unwrap();
    assert!(install_stub_at(stub(home.path())).unwrap().installed);
    assert!(!std::fs::read_to_string(stub(home.path())).unwrap().contains("xxxx"));
}

#[test]
fn a_repair_rewrites_only_an_older_or_unversioned_stub() {
    let home = tempfile::tempdir().unwrap();
    assert!(!repair_stub_at(stub(home.path())).unwrap().rewritten);
    std::fs::create_dir_all(stub(home.path()).parent().unwrap()).unwrap();
    let repaired = |contents: &str| {
        std::fs::write(stub(home.path()), contents).unwrap();
        let rewritten = repair_stub_at(stub(home.path())).unwrap().rewritten;
        (rewritten, std::fs::read_to_string(stub(home.path())).unwrap() == contents)
    };
    assert_eq!(
        [
            repaired(&stub_of_version("0.0.1")),
            repaired("plain"),
            repaired(&stub_of_version("99.0.0")),
            repaired(&stub_of_version(VERSION))
        ],
        [(true, false), (true, false), (false, true), (false, true)]
    );
}

#[test]
fn the_skill_path_writes_every_skill_for_this_version_and_drops_older_versions() {
    let home = tempfile::tempdir().unwrap();
    let cache = home.path().join("skills");
    std::fs::create_dir_all(cache.join("0.0.1/core")).unwrap();
    std::fs::create_dir_all(cache.join("99.0.0")).unwrap();
    std::fs::write(cache.join("0.0.2"), "a file, not a version folder").unwrap();
    std::fs::create_dir_all(cache.join("notes")).unwrap();
    let root = cache.join(VERSION);
    assert_eq!(path_in(home.path(), None, false).unwrap(), format!("{}\n", root.display()));
    assert_eq!(
        path_in(home.path(), Some("tui-qa"), true).unwrap(),
        format!("{}\n", serde_json::json!({ "path": root.join("tui-qa") }))
    );
    let mut left: Vec<String> = std::fs::read_dir(&cache)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    let mut expected = vec!["0.0.2".to_string(), "99.0.0".to_string(), VERSION.to_string(), "notes".to_string()];
    expected.sort();
    assert_eq!(left, expected);
    assert!(
        root.join("tui-qa/templates/report.md").is_file()
            && root.join("core/SKILL.md").is_file()
            && root.join("agent-terminal/SKILL.md").is_file()
    );
    assert!(path_in(home.path(), Some("nope"), false).is_err());
}

#[test]
fn a_stub_that_cannot_be_opened_is_an_error_rather_than_missing() {
    let home = tempfile::tempdir().unwrap();
    let file = home.path().join("file");
    std::fs::write(&file, "").unwrap();
    let inside = file.join("SKILL.md");
    let refused = installed_stub_at(inside.clone()).err().map(|e| e.to_string());
    assert_eq!(refused, Some(format!("open {}", inside.display())));
}

#[test]
fn removing_a_version_folder_ignores_one_already_gone_but_not_one_it_cannot_remove() {
    let home = tempfile::tempdir().unwrap();
    let file = home.path().join("1.0.0");
    std::fs::write(&file, "").unwrap();
    assert!(super::remove_version(&home.path().join("missing")).is_ok());
    assert_eq!(super::remove_version(&file).unwrap_err().to_string(), format!("remove {}", file.display()));
}

#[test]
fn the_command_reference_leaves_out_hidden_commands_and_help() {
    let cli = Command::new("agent-terminal")
        .subcommand(Command::new("open").about("Run it"))
        .subcommand(Command::new("secret").about("Kept out").hide(true));
    let full = get("core", true, false, cli).unwrap();
    assert!(full.contains("Usage: agent-terminal open") && !full.contains("Kept out"), "{full}");
    assert!(!full.contains("Usage: agent-terminal help"), "{full}");
}
