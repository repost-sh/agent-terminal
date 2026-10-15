use std::path::Path;

use super::install::installed_stub;
use super::shipped::shipped;
use crate::support::{AgentTerminal, VERSION};

fn frontmatter_keys(skill: &str) -> Vec<&str> {
    let (front, _) = skill.strip_prefix("---\n").and_then(|rest| rest.split_once("\n---\n")).expect("frontmatter");
    front.lines().filter(|line| !line.starts_with(' ')).filter_map(|line| Some(line.split_once(':')?.0)).collect()
}

#[test]
fn the_stub_is_not_hidden_so_opencode_and_every_other_assistant_loads_it() {
    let at = AgentTerminal::new();
    assert_eq!(frontmatter_keys(&installed_stub(&at)), ["name", "description", "allowed-tools", "metadata"]);
}

fn yaml_problem(value: &str) -> Option<&'static str> {
    if let Some(quote) = value.chars().next().filter(|first| ['\'', '"'].contains(first)) {
        return (value.len() < 2 || !value.ends_with(quote)).then_some("a quoted value ends with its quote");
    }
    if value.starts_with(['[', '{', '>', '|', '*', '&', '!', '%', '@', '`', '#']) {
        return Some("a plain value starts with a YAML indicator");
    }
    if value.contains(": ") {
        return Some("\": \" in a plain value starts a nested mapping");
    }
    value.contains(" #").then_some("\" #\" in a plain value starts a comment")
}

#[test]
fn every_skills_frontmatter_is_yaml_that_npx_skills_add_and_other_strict_readers_parse() {
    let at = AgentTerminal::new();
    let mut problems = Vec::new();
    for name in ["agent-terminal", "core", "tui-qa", "cli-qa"] {
        let guide = at.run(&["skills", "get", name]).success().stdout();
        let (front, _) = guide.strip_prefix("---\n").and_then(|rest| rest.split_once("\n---\n")).expect("frontmatter");
        for line in front.lines().filter(|line| !line.starts_with(' ') && !line.ends_with(':')) {
            match line.split_once(": ") {
                Some((key, value)) => {
                    problems.extend(yaml_problem(value).map(|problem| format!("{name}: {key}: {problem}")));
                }
                None => problems.push(format!("{name}: `{line}` is not `key: value`")),
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn npx_skills_add_finds_one_skill_the_versioned_stub_that_install_writes() {
    let at = AgentTerminal::new();
    let skills = Path::new(env!("CARGO_MANIFEST_DIR")).join("../skills");
    let found: Vec<String> =
        std::fs::read_dir(skills).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    assert_eq!(found, ["agent-terminal"]);
    assert_eq!(
        shipped("skills/agent-terminal/SKILL.md"),
        installed_stub(&at),
        "skills/agent-terminal/SKILL.md must carry metadata.version {VERSION}, the version in cli/Cargo.toml"
    );
}

#[test]
fn the_plugin_marketplace_offers_one_plugin_holding_the_stub_that_install_writes() {
    let at = AgentTerminal::new();
    let marketplace: serde_json::Value = serde_json::from_str(&shipped(".claude-plugin/marketplace.json")).unwrap();
    let plugins = marketplace["plugins"].as_array().expect("plugins");
    let names: Vec<&str> = plugins.iter().map(|plugin| plugin["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["agent-terminal"]);
    let source = Path::new(plugins[0]["source"].as_str().unwrap());
    let skills: Vec<String> = plugins[0]["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|skill| shipped(source.join(skill.as_str().unwrap()).join("SKILL.md").to_str().unwrap()))
        .collect();
    assert_eq!(skills, [installed_stub(&at)]);
}
