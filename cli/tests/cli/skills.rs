use std::path::{Path, PathBuf};

use crate::support::{AgentTerminal, VERSION};

pub fn shipped(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

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
        &["open", "-h"],
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

pub fn stub_in(user: &Path) -> PathBuf {
    user.join(".claude/skills/agent-terminal/SKILL.md")
}

pub fn stub_of_version(version: &str) -> String {
    format!(
        "---\nname: agent-terminal\ndescription: The stub of agent-terminal {version}\nmetadata:\n  version: \"{version}\"\n---\n\n# agent-terminal {version}\n"
    )
}

pub fn put_stub(user: &Path, contents: &str) {
    let stub = stub_in(user);
    std::fs::create_dir_all(stub.parent().unwrap()).unwrap();
    std::fs::write(stub, contents).unwrap();
}

#[test]
fn install_skills_writes_the_discovery_stub_into_the_agent_skills_folder() {
    let at = AgentTerminal::new();
    let user = at.home().join("user");
    let run = at.run_with_env(&["install", "--skills"], &[("HOME", &user)]).success();
    let stub = stub_in(&user);
    assert_eq!(run.stdout(), format!("installed the agent-terminal skill {VERSION} at {}\n", stub.display()));
    let installed = std::fs::read_to_string(&stub).unwrap();
    assert_eq!(installed, at.run(&["skills", "get", "agent-terminal"]).success().stdout());
    let shipped = shipped("skills/agent-terminal/SKILL.md");
    let body = shipped.split_once("\n---\n").expect("frontmatter").1;
    assert!(installed.starts_with("---\nname: agent-terminal\n"), "{installed}");
    assert!(installed.contains("\nInstall: `npm i -g @repost/agent-terminal`\n"), "{installed}");
    assert!(
        installed.ends_with(&format!(
            "\nallowed-tools: Bash(agent-terminal:*), Bash(npx @repost/agent-terminal:*)\nmetadata:\n  version: \
             \"{VERSION}\"\n---\n{body}"
        )),
        "{installed}"
    );
}

fn frontmatter_keys(skill: &str) -> Vec<&str> {
    let (front, _) = skill.strip_prefix("---\n").and_then(|rest| rest.split_once("\n---\n")).expect("frontmatter");
    front.lines().filter(|line| !line.starts_with(' ')).filter_map(|line| Some(line.split_once(':')?.0)).collect()
}

fn installed_stub(at: &AgentTerminal) -> String {
    let user = at.home().join("user");
    at.run_with_env(&["install", "--skills"], &[("HOME", &user)]).success();
    std::fs::read_to_string(stub_in(&user)).unwrap()
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
                    problems.extend(yaml_problem(value).map(|problem| format!("{name}: {key}: {problem}")))
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

#[test]
fn install_without_flags_installs_the_stub_too() {
    let at = AgentTerminal::new();
    let user = at.home().join("user");
    at.run_with_env(&["install"], &[("HOME", &user)]).success();
    let installed = std::fs::read_to_string(stub_in(&user)).unwrap();
    assert_eq!(installed, at.run(&["skills", "get", "agent-terminal"]).success().stdout());
}

#[test]
fn install_skills_keeps_a_newer_stub() {
    let at = AgentTerminal::new();
    let user = at.home().join("user");
    put_stub(&user, &stub_of_version("999.0.0"));
    let run = at.run_with_env(&["install", "--skills"], &[("HOME", &user)]).success();
    let stub = stub_in(&user);
    assert_eq!(
        run.stdout(),
        format!("kept the newer agent-terminal skill 999.0.0 at {} (this is {VERSION})\n", stub.display())
    );
    assert_eq!(std::fs::read_to_string(stub).unwrap(), stub_of_version("999.0.0"));
}

#[test]
fn install_skills_replaces_an_older_or_unversioned_stub() {
    let at = AgentTerminal::new();
    let user = at.home().join("user");
    let current = at.run(&["skills", "get", "agent-terminal"]).success().stdout();
    for existing in [stub_of_version("0.0.1"), stub_of_version("0.1.0-alpha.1"), "# my notes\n".to_string()] {
        put_stub(&user, &existing);
        let run = at.run_with_env(&["install", "--skills"], &[("HOME", &user)]).success();
        assert_eq!(
            run.stdout(),
            format!("installed the agent-terminal skill {VERSION} at {}\n", stub_in(&user).display())
        );
        assert_eq!(std::fs::read_to_string(stub_in(&user)).unwrap(), current, "replacing:\n{existing}");
    }
}

#[test]
fn install_skills_as_json() {
    let at = AgentTerminal::new();
    let user = at.home().join("user");
    let stub = stub_in(&user);
    let installed = at.run_with_env(&["--json", "install", "--skills"], &[("HOME", &user)]).success().json();
    assert_eq!(installed, serde_json::json!({ "installed": true, "path": stub, "version": VERSION }));
    put_stub(&user, &stub_of_version("999.0.0"));
    let kept = at.run_with_env(&["--json", "install", "--skills"], &[("HOME", &user)]).success().json();
    assert_eq!(kept, serde_json::json!({ "installed": false, "path": stub, "version": "999.0.0" }));
}

#[test]
fn simultaneous_installs_leave_one_complete_stub() {
    let at = AgentTerminal::new();
    let user = at.home().join("user");
    std::thread::scope(|scope| {
        let installs: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| at.run_with_env(&["install", "--skills"], &[("HOME", &user)]).code()))
            .collect();
        let codes: Vec<i32> = installs.into_iter().map(|install| install.join().unwrap()).collect();
        assert_eq!(codes, vec![0; 8]);
    });
    let installed = std::fs::read_to_string(stub_in(&user)).unwrap();
    assert_eq!(installed, at.run(&["skills", "get", "agent-terminal"]).success().stdout());
}

fn code_in(markdown: &str) -> Vec<&str> {
    let mut code = Vec::new();
    let mut fenced = false;
    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if fenced {
            code.push(line);
        } else {
            code.extend(line.split('`').skip(1).step_by(2));
        }
    }
    code
}

fn commands_run_in(code: &str) -> Vec<Vec<&str>> {
    code.match_indices("agent-terminal ")
        .filter(|(at, _)| !code[..*at].ends_with(|c: char| c.is_alphanumeric() || "-_/.".contains(c)))
        .map(|(at, name)| code[at + name.len()..].split_whitespace().collect::<Vec<_>>())
        .filter(|words| words.first().is_some_and(|first| !first.starts_with('<')))
        .collect()
}

struct Help {
    subcommands: Vec<String>,
    arguments: bool,
}

struct Commands<'a> {
    at: &'a AgentTerminal,
    known: std::collections::HashMap<Vec<String>, Option<Help>>,
}

impl Commands<'_> {
    fn help(&mut self, path: &[String]) -> Option<&Help> {
        let at = self.at;
        self.known
            .entry(path.to_vec())
            .or_insert_with(|| {
                let mut args = vec!["help"];
                args.extend(path.iter().map(String::as_str));
                let run = at.run(&args);
                (run.code() == 0).then(|| {
                    let printed = run.stdout();
                    let listed = printed.split("\nCommands:\n").nth(1).unwrap_or_default();
                    let subcommands = listed
                        .lines()
                        .take_while(|line| line.starts_with("  "))
                        .filter_map(|line| line.split_whitespace().next())
                        .filter(|name| *name != "help")
                        .map(String::from)
                        .collect();
                    Help { subcommands, arguments: printed.contains("\nArguments:\n") }
                })
            })
            .as_ref()
    }

    fn path(&mut self, words: &[&str]) -> Result<Vec<String>, String> {
        let mut path = vec![words[0].to_string()];
        for word in &words[1..] {
            let help =
                self.help(&path).ok_or_else(|| format!("`{}` is not a command of agent-terminal", path.join(" ")))?;
            if help.subcommands.iter().any(|name| name == word) {
                path.push(word.to_string());
                continue;
            }
            if !help.subcommands.is_empty() && !help.arguments && !word.starts_with(['-', '<', '"', '\'']) {
                return Err(format!("`{}` has no subcommand `{word}`", path.join(" ")));
            }
            return Ok(path);
        }
        self.help(&path).ok_or_else(|| format!("`{}` is not a command of agent-terminal", path.join(" ")))?;
        Ok(path)
    }
}

#[test]
fn every_command_a_guide_runs_is_a_command_of_this_binary() {
    let at = AgentTerminal::new();
    let mut names: Vec<String> = at.run(&["--json", "skills", "list"]).success().json()["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|skill| skill["name"].as_str().unwrap().to_string())
        .collect();
    names.push("agent-terminal".into());

    let mut commands = Commands { at: &at, known: std::collections::HashMap::new() };
    let mut used = std::collections::BTreeMap::<String, Vec<String>>::new();
    let mut problems = Vec::new();
    for name in &names {
        let guide = at.run(&["skills", "get", name]).success().stdout();
        for code in code_in(&guide) {
            for words in commands_run_in(code) {
                let place = format!("{name}: `{code}`");
                if words[0].starts_with('-') {
                    problems.push(format!("put the command before its options in {place}"));
                    continue;
                }
                match commands.path(&words) {
                    Ok(path) => used.entry(path.join(" ")).or_default().push(place),
                    Err(problem) => problems.push(format!("{problem}, but {place} runs it")),
                }
            }
        }
    }
    for built in ["open", "snapshot", "close", "skills get", "history show", "diff screenshot", "tab new"] {
        assert!(used.contains_key(built), "no guide runs `{built}`; is the guide scan broken?\n{used:#?}");
    }
    assert!(problems.is_empty(), "guides and the commands of this binary disagree:\n{}", problems.join("\n"));
}

#[test]
fn the_help_of_every_command_calls_a_terminal_a_terminal_or_a_tab() {
    let at = AgentTerminal::new();
    let mut commands = Commands { at: &at, known: std::collections::HashMap::new() };
    let mut paths: Vec<Vec<String>> = vec![Vec::new()];
    let mut banned = Vec::new();
    while let Some(path) = paths.pop() {
        let mut args = vec!["help"];
        args.extend(path.iter().map(String::as_str));
        let text = at.run(&args).success().stdout().to_lowercase();
        for word in ["window", "pane"] {
            if text.split(|c: char| !c.is_alphanumeric()).any(|said| said == word || said == format!("{word}s")) {
                banned.push(format!("`help {}` says {word}", path.join(" ")));
            }
        }
        let subcommands = commands.help(&path).map(|help| help.subcommands.clone()).unwrap_or_default();
        paths.extend(subcommands.into_iter().map(|name| [path.clone(), vec![name]].concat()));
    }
    assert!(banned.is_empty(), "GLOSSARY.md says terminal or tab, never pane or window:\n{}", banned.join("\n"));
}

#[test]
fn a_guide_that_runs_a_renamed_subcommand_fails_the_check() {
    let at = AgentTerminal::new();
    let mut commands = Commands { at: &at, known: std::collections::HashMap::new() };
    assert_eq!(commands.path(&["history", "show", "3"]), Ok(vec!["history".to_string(), "show".to_string()]));
    assert_eq!(commands.path(&["tab", "t2"]), Ok(vec!["tab".to_string()]));
    assert_eq!(commands.path(&["history", "detail", "3"]), Err("`history` has no subcommand `detail`".to_string()));
    assert_eq!(commands.path(&["get", "txt"]), Err("`get` has no subcommand `txt`".to_string()));
    assert_eq!(
        commands.path(&["diff", "screen", "--baseline", "a.png"]),
        Err("`diff` has no subcommand `screen`".to_string())
    );
    assert_eq!(commands.path(&["snap"]), Err("`snap` is not a command of agent-terminal".to_string()));
}

fn the_setup_copies_the_report_template(skill: &str) {
    let at = AgentTerminal::new();
    let guide = at.run(&["skills", "get", skill]).success().stdout();
    let copy = code_in(&guide)
        .into_iter()
        .find(|line| line.contains("templates/report.md"))
        .unwrap_or_else(|| panic!("the {skill} guide copies its report template"));
    let work = at.home().join("work");
    std::fs::create_dir_all(work.join(format!("{skill}-output"))).unwrap();
    let bin = Path::new(env!("CARGO_BIN_EXE_agent-terminal")).parent().unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let run = std::process::Command::new("sh")
        .args(["-c", copy])
        .current_dir(&work)
        .env("PATH", path)
        .env("AGENT_TERMINAL_HOME", at.home())
        .output()
        .unwrap();
    assert!(run.status.success(), "`{copy}` failed:\n{}", String::from_utf8_lossy(&run.stderr));
    assert_eq!(
        std::fs::read_to_string(work.join(format!("{skill}-output/report.md"))).unwrap(),
        shipped(&format!("skill-data/{skill}/templates/report.md"))
    );
}

#[test]
fn the_tui_qa_setup_copies_the_report_template() {
    the_setup_copies_the_report_template("tui-qa");
}

#[test]
fn the_cli_qa_setup_copies_the_report_template() {
    the_setup_copies_the_report_template("cli-qa");
}
