use std::collections::{BTreeMap, HashMap};

use crate::support::AgentTerminal;

pub(super) fn code_in(markdown: &str) -> Vec<&str> {
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
    known: HashMap<Vec<String>, Option<Help>>,
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

struct GuideScan<'a> {
    commands: Commands<'a>,
    used: BTreeMap<String, Vec<String>>,
    problems: Vec<String>,
}

impl GuideScan<'_> {
    fn scan(&mut self, name: &str, guide: &str) {
        for code in code_in(guide) {
            for words in commands_run_in(code) {
                self.check(format!("{name}: `{code}`"), &words);
            }
        }
    }

    fn check(&mut self, place: String, words: &[&str]) {
        if words[0].starts_with('-') {
            self.problems.push(format!("put the command before its options in {place}"));
            return;
        }
        match self.commands.path(words) {
            Ok(path) => self.used.entry(path.join(" ")).or_default().push(place),
            Err(problem) => self.problems.push(format!("{problem}, but {place} runs it")),
        }
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

    let commands = Commands { at: &at, known: HashMap::new() };
    let mut scan = GuideScan { commands, used: BTreeMap::new(), problems: Vec::new() };
    for name in &names {
        let guide = at.run(&["skills", "get", name]).success().stdout();
        scan.scan(name, &guide);
    }
    let (used, problems) = (scan.used, scan.problems);
    for built in ["open", "snapshot", "close", "skills get", "history show", "diff screenshot", "tab new"] {
        assert!(used.contains_key(built), "no guide runs `{built}`; is the guide scan broken?\n{used:#?}");
    }
    assert!(problems.is_empty(), "guides and the commands of this binary disagree:\n{}", problems.join("\n"));
}

#[test]
fn the_help_of_every_command_calls_a_terminal_a_terminal_or_a_tab() {
    let at = AgentTerminal::new();
    let mut commands = Commands { at: &at, known: HashMap::new() };
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
    assert!(banned.is_empty(), "the help says terminal or tab, never pane or window:\n{}", banned.join("\n"));
}

#[test]
fn a_guide_that_runs_a_renamed_subcommand_fails_the_check() {
    let at = AgentTerminal::new();
    let mut commands = Commands { at: &at, known: HashMap::new() };
    assert_eq!(commands.path(&["history", "show", "3"]), Ok(vec!["history".to_string(), "show".to_string()]));
    assert_eq!(commands.path(&["tab", "api"]), Ok(vec!["tab".to_string()]));
    a_renamed_or_unknown_command_is_named(&mut commands);
}

fn a_renamed_or_unknown_command_is_named(commands: &mut Commands<'_>) {
    assert_eq!(commands.path(&["history", "detail", "3"]), Err("`history` has no subcommand `detail`".to_string()));
    assert_eq!(commands.path(&["get", "txt"]), Err("`get` has no subcommand `txt`".to_string()));
    assert_eq!(
        commands.path(&["diff", "screen", "--baseline", "a.png"]),
        Err("`diff` has no subcommand `screen`".to_string())
    );
    assert_eq!(commands.path(&["snap"]), Err("`snap` is not a command of agent-terminal".to_string()));
}
