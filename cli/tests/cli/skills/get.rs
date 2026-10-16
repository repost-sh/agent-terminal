use super::shipped::shipped;
use crate::support::{AgentTerminal, VERSION};

#[test]
fn skills_lists_the_guides_with_one_line_descriptions() {
    let at = AgentTerminal::new();
    let expected = "\
core    Core agent-terminal usage guide.
tui-qa  Systematically test a full-screen terminal app (TUI) and find bugs, broken layouts, dead keys and confusing flows.
cli-qa  Test a command-line tool the way users run it, from a real terminal.
";
    assert_eq!(at.run(&["skills"]).success().stdout(), expected);
    assert_eq!(at.run(&["skills", "list"]).success().stdout(), expected);
}

#[test]
fn skills_list_as_json_gives_full_descriptions() {
    let at = AgentTerminal::new();
    let list = at.run(&["--json", "skills", "list"]).success().json();
    let skills = list["skills"].as_array().expect("skills array");
    let names: Vec<&str> = skills.iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["core", "tui-qa", "cli-qa"]);
    assert_eq!(
        skills[2]["description"],
        "Test a command-line tool the way users run it, from a real terminal. Use when asked to QA or review a CLI's \
         help text, errors, exit codes, prompts, colors, or behavior when piped. Produces a report with the exact \
         commands, their output and exit codes."
    );
}

#[test]
fn skills_get_prints_the_shipped_guide_stamped_with_this_version() {
    let at = AgentTerminal::new();
    let guide = at.run(&["skills", "get", "cli-qa"]).success().stdout();
    let shipped = shipped("skill-data/cli-qa/SKILL.md");
    let body = shipped.split_once("\n---\n").expect("frontmatter").1;
    let expected = format!(
        "---\n\
         name: cli-qa\n\
         description: Test a command-line tool the way users run it, from a real terminal. Use when asked to QA or \
         review a CLI's help text, errors, exit codes, prompts, colors, or behavior when piped. Produces a report \
         with the exact commands, their output and exit codes.\n\
         allowed-tools: Bash(agent-terminal:*), Bash(npx @repost/agent-terminal:*)\n\
         metadata:\n  version: \"{VERSION}\"\n\
         ---\n{body}"
    );
    assert_eq!(guide, expected);
}

#[test]
fn every_skill_allows_the_command_and_npx_of_the_repost_package() {
    let at = AgentTerminal::new();
    for name in ["agent-terminal", "core", "tui-qa", "cli-qa"] {
        let guide = at.run(&["skills", "get", name]).success().stdout();
        assert!(
            guide.contains("\nallowed-tools: Bash(agent-terminal:*), Bash(npx @repost/agent-terminal:*)\n"),
            "{name}:\n{guide}"
        );
    }
}

#[test]
fn skills_get_core_full_appends_the_full_command_reference() {
    let at = AgentTerminal::new();
    let guide = at.run(&["skills", "get", "core"]).success().stdout();
    let full = at.run(&["skills", "get", "core", "--full"]).success().stdout();
    let reference = full.strip_prefix(guide.as_str()).expect("--full starts with the guide");
    assert!(reference.starts_with("\n--- command reference ---\n\n"), "{reference}");
    for help in [
        &["-h"][..],
        &["open", "--name", "app", "-h"],
        &["snapshot", "-h"],
        &["close", "-h"],
        &["skills", "-h"],
        &["skills", "get", "-h"],
        &["install", "-h"],
    ] {
        let text = at.run(help).success().stdout();
        assert!(reference.contains(&text), "the reference lacks `agent-terminal {}`:\n{text}", help.join(" "));
    }
}

#[test]
fn skills_get_full_appends_a_skills_other_files() {
    let at = AgentTerminal::new();
    let guide = at.run(&["skills", "get", "tui-qa"]).success().stdout();
    let full = at.run(&["skills", "get", "tui-qa", "--full"]).success().stdout();
    let template = shipped("skill-data/tui-qa/templates/report.md");
    assert_eq!(full, format!("{guide}\n--- templates/report.md ---\n\n{template}"));
}

#[test]
fn skills_get_as_json_gives_name_and_content() {
    let at = AgentTerminal::new();
    let guide = at.run(&["skills", "get", "core"]).success().stdout();
    let json = at.run(&["--json", "skills", "get", "core"]).success().json();
    assert_eq!(json, serde_json::json!({ "name": "core", "content": guide }));
}

#[test]
fn skills_get_an_unknown_skill_fails_and_says_how_to_list_them() {
    let at = AgentTerminal::new();
    let run = at.run(&["skills", "get", "nope"]);
    assert_eq!(run.code(), 1);
    assert_eq!(run.stderr(), "error: no skill named nope; list them with `agent-terminal skills list`\n");
}

#[test]
fn no_guide_in_its_full_form_speaks_of_the_daemon() {
    let at = AgentTerminal::new();
    let list = at.run(&["--json", "skills", "list"]).success().json();
    let skills = list["skills"].as_array().expect("skills array");
    for name in skills.iter().map(|skill| skill["name"].as_str().expect("a skill name")) {
        let full = at.run(&["skills", "get", name, "--full"]).success().stdout();
        let said: Vec<&str> = full.lines().filter(|line| line.to_lowercase().contains("daemon")).collect();
        assert!(said.is_empty(), "`skills get {name} --full` speaks of the daemon:\n{}", said.join("\n"));
    }
}
