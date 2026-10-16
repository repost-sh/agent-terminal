use clap::Command;

use super::{SKILLS, find, get, list, path_in};
use crate::version::VERSION;

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
