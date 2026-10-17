use std::fmt::Write as _;
use std::io::{self, ErrorKind};
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use clap::Subcommand;
use semver::Version;
use serde::Serialize;

use crate::home::home;
use crate::output;
use crate::protocol::ErrorBody;
use crate::version::VERSION;

#[derive(Subcommand)]
pub(crate) enum Action {
    #[command(about = "The skills in this version, one line each")]
    List,
    #[command(about = "Print a skill's guide")]
    Get {
        #[arg(help = "The skill, e.g. core", value_name = "NAME")]
        name: String,
        #[arg(long, help = "Append the skill's other files (for core: the full command reference)")]
        full: bool,
    },
    #[command(about = "Where a skill's files are on disk (without a name: the folder of every skill)")]
    Path {
        #[arg(help = "The skill, e.g. tui-qa", value_name = "NAME")]
        name: Option<String>,
    },
}

struct Skill {
    name: &'static str,
    guide: &'static str,
    extras: &'static [(&'static str, &'static str)],
}

const GUIDE: &str = "SKILL.md";

const SKILLS: &[Skill] = &[
    Skill { name: "agent-terminal", guide: include_str!("../../../skills/agent-terminal/SKILL.md"), extras: &[] },
    Skill { name: "core", guide: include_str!("../../../skill-data/core/SKILL.md"), extras: &[] },
    Skill {
        name: "tui-qa",
        guide: include_str!("../../../skill-data/tui-qa/SKILL.md"),
        extras: &[("templates/report.md", include_str!("../../../skill-data/tui-qa/templates/report.md"))],
    },
    Skill {
        name: "cli-qa",
        guide: include_str!("../../../skill-data/cli-qa/SKILL.md"),
        extras: &[("templates/report.md", include_str!("../../../skill-data/cli-qa/templates/report.md"))],
    },
];

impl Skill {
    fn sections(&self) -> Result<(&'static str, &'static str)> {
        self.guide
            .strip_prefix("---\n")
            .and_then(|guide| guide.split_once("\n---\n"))
            .with_context(|| format!("the built-in skill {} does not start with frontmatter", self.name))
    }

    fn versioned_guide(&self) -> Result<String> {
        let (front, body) = self.sections()?;
        let fields = front.split_once("\nmetadata:\n").map_or(front, |(fields, _)| fields);
        Ok(format!("---\n{fields}\nmetadata:\n  version: \"{VERSION}\"\n---\n{body}"))
    }

    fn files(&self) -> Result<Vec<(&'static str, String)>> {
        let mut files = vec![(GUIDE, self.versioned_guide()?)];
        files.extend(self.extras.iter().map(|(path, text)| (*path, (*text).to_string())));
        Ok(files)
    }

    fn description(&self) -> Result<&'static str> {
        let (front, _) = self.sections()?;
        front
            .lines()
            .find_map(|line| line.strip_prefix("description")?.strip_prefix(": "))
            .with_context(|| format!("the built-in skill {} has no description", self.name))
    }

    fn summary(&self) -> Result<&'static str> {
        let description = self.description()?;
        Ok(description.find(". ").and_then(|end| description.get(..=end)).unwrap_or(description))
    }

    fn is_stub(&self) -> bool {
        self.name == "agent-terminal"
    }
}

impl Action {
    pub(crate) fn run(self, json: bool, cli: clap::Command) -> io::Result<i32> {
        let printed = match self {
            Action::List => list(json),
            Action::Get { name, full } => get(&name, full, json, cli),
            Action::Path { name } => path(name.as_deref(), json),
        };
        finish(printed, json)
    }
}

fn finish(printed: Result<String>, json: bool) -> io::Result<i32> {
    match printed {
        Ok(text) => output::print(&text).map(|()| 0),
        Err(e) => output::error(&ErrorBody::failure(format!("{e:#}")), json),
    }
}

fn find(name: &str) -> Result<&'static Skill> {
    SKILLS
        .iter()
        .find(|skill| skill.name == name)
        .ok_or_else(|| anyhow!("no skill named {name}; list them with `agent-terminal skills list`"))
}

fn list(json: bool) -> Result<String> {
    let listed: Vec<&Skill> = SKILLS.iter().filter(|skill| !skill.is_stub()).collect();
    if json {
        let skills = listed
            .iter()
            .map(|skill| Ok(serde_json::json!({ "name": skill.name, "description": skill.description()? })))
            .collect::<Result<Vec<_>>>()?;
        return Ok(format!("{}\n", serde_json::json!({ "skills": skills })));
    }
    let width = listed.iter().map(|skill| skill.name.len()).max().unwrap_or(0);
    let mut text = String::new();
    for skill in listed {
        writeln!(text, "{:<width$}  {}", skill.name, skill.summary()?)?;
    }
    Ok(text)
}

fn get(name: &str, full: bool, json: bool, cli: clap::Command) -> Result<String> {
    let skill = find(name)?;
    let mut content = skill.versioned_guide()?;
    if full {
        for (path, text) in skill.extras {
            write!(content, "\n--- {path} ---\n\n{text}")?;
        }
        if skill.name == "core" {
            write!(content, "\n--- command reference ---\n\n{}", reference(cli))?;
        }
    }
    if json {
        return Ok(format!("{}\n", serde_json::json!({ "name": skill.name, "content": content })));
    }
    Ok(content)
}

#[derive(Serialize)]
struct SkillPath<'a> {
    path: &'a Path,
}

fn path(name: Option<&str>, json: bool) -> Result<String> {
    path_in(&home()?, name, json)
}

fn path_in(home: &Path, name: Option<&str>, json: bool) -> Result<String> {
    let skill = name.map(find).transpose()?;
    let cache = home.join("skills");
    let root = cache.join(VERSION);
    for each in SKILLS {
        for (relative, text) in each.files()? {
            write_atomically(&root.join(each.name).join(relative), &text)?;
        }
    }
    remove_older_versions(&cache)?;
    let path = skill.map_or_else(|| root.clone(), |skill| root.join(skill.name));
    if json {
        return Ok(format!("{}\n", serde_json::to_value(SkillPath { path: &path })?));
    }
    Ok(format!("{}\n", path.display()))
}

fn remove_older_versions(cache: &Path) -> Result<()> {
    let ours = crate::version::this_version()?;
    for entry in std::fs::read_dir(cache).with_context(|| format!("read {}", cache.display()))? {
        let entry = entry.with_context(|| format!("read {}", cache.display()))?;
        if is_an_older_version(&entry, &ours)? {
            remove_version(&entry.path())?;
        }
    }
    Ok(())
}

fn is_an_older_version(entry: &std::fs::DirEntry, ours: &Version) -> Result<bool> {
    let older =
        entry.file_name().to_str().and_then(|name| Version::parse(name).ok()).is_some_and(|theirs| theirs < *ours);
    Ok(older && entry.file_type().with_context(|| format!("inspect {}", entry.path().display()))?.is_dir())
}

fn remove_version(folder: &Path) -> Result<()> {
    match std::fs::remove_dir_all(folder) {
        Err(e) if e.kind() != ErrorKind::NotFound => Err(e).with_context(|| format!("remove {}", folder.display())),
        _ => Ok(()),
    }
}

fn write_atomically(path: &Path, contents: &str) -> Result<()> {
    let folder = path.parent().context("a skill file has a folder")?;
    std::fs::create_dir_all(folder).with_context(|| format!("create {}", folder.display()))?;
    let partial = path.with_extension(format!("{}.partial", std::process::id()));
    std::fs::write(&partial, contents).with_context(|| format!("write {}", partial.display()))?;
    std::fs::rename(&partial, path).with_context(|| format!("write {}", path.display()))
}

fn reference(mut cli: clap::Command) -> String {
    cli.build();
    let mut sections = Vec::new();
    collect_help(&mut cli, &mut sections);
    sections.join("\n")
}

fn collect_help(command: &mut clap::Command, sections: &mut Vec<String>) {
    sections.push(command.render_help().to_string());
    for sub in command.get_subcommands_mut().filter(|sub| !sub.is_hide_set() && sub.get_name() != "help") {
        collect_help(sub, sections);
    }
}

#[cfg(test)]
mod tests;
