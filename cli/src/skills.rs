use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::Subcommand;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::connection;
use crate::output;
use crate::protocol::ErrorBody;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Subcommand)]
pub enum Action {
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
    shipped: &'static [(&'static str, &'static str)],
}

const SKILLS: &[Skill] = &[
    Skill { name: "agent-terminal", shipped: &[("SKILL.md", include_str!("../../skills/agent-terminal/SKILL.md"))] },
    Skill { name: "core", shipped: &[("SKILL.md", include_str!("../../skill-data/core/SKILL.md"))] },
    Skill {
        name: "tui-qa",
        shipped: &[
            ("SKILL.md", include_str!("../../skill-data/tui-qa/SKILL.md")),
            ("templates/report.md", include_str!("../../skill-data/tui-qa/templates/report.md")),
        ],
    },
    Skill {
        name: "cli-qa",
        shipped: &[
            ("SKILL.md", include_str!("../../skill-data/cli-qa/SKILL.md")),
            ("templates/report.md", include_str!("../../skill-data/cli-qa/templates/report.md")),
        ],
    },
];

impl Skill {
    fn sections(&self) -> (&'static str, &'static str) {
        self.shipped[0]
            .1
            .strip_prefix("---\n")
            .and_then(|guide| guide.split_once("\n---\n"))
            .expect("every built-in skill starts with frontmatter")
    }

    fn guide(&self) -> String {
        let (front, body) = self.sections();
        let fields = front.split_once("\nmetadata:\n").map_or(front, |(fields, _)| fields);
        format!("---\n{fields}\nmetadata:\n  version: \"{VERSION}\"\n---\n{body}")
    }

    fn files(&self) -> Vec<(&'static str, String)> {
        let mut files = vec![(self.shipped[0].0, self.guide())];
        files.extend(self.shipped[1..].iter().map(|(path, text)| (*path, text.to_string())));
        files
    }

    fn frontmatter(&self, key: &str) -> Option<&'static str> {
        self.sections().0.lines().find_map(|line| line.strip_prefix(key)?.strip_prefix(": "))
    }

    fn description(&self) -> &'static str {
        self.frontmatter("description").expect("every built-in skill has a description")
    }

    fn summary(&self) -> &'static str {
        let description = self.description();
        description.find(". ").map_or(description, |end| &description[..=end])
    }

    fn is_stub(&self) -> bool {
        self.name == "agent-terminal"
    }
}

pub fn run(action: Option<Action>, json: bool, cli: clap::Command) -> i32 {
    let result = match action.unwrap_or(Action::List) {
        Action::List => {
            list(json);
            Ok(())
        }
        Action::Get { name, full } => get(&name, full, json, cli),
        Action::Path { name } => path(name.as_deref(), json),
    };
    finish(result, json)
}

pub fn install(json: bool) -> i32 {
    let result = install_stub().map(|outcome| {
        if json {
            println!("{}", serde_json::json!(outcome));
        } else if outcome.installed {
            println!("installed the agent-terminal skill {} at {}", outcome.version, outcome.path.display());
        } else {
            println!(
                "kept the newer agent-terminal skill {} at {} (this is {VERSION})",
                outcome.version,
                outcome.path.display()
            );
        }
    });
    finish(result, json)
}

fn finish(result: Result<()>, json: bool) -> i32 {
    match result {
        Ok(()) => 0,
        Err(e) => output::error(
            &ErrorBody { code: crate::exit::ERROR, message: format!("{e:#}"), snapshot: None, other_version: None },
            json,
        ),
    }
}

#[derive(Serialize, Deserialize)]
pub struct Installed {
    pub installed: bool,
    pub path: PathBuf,
    pub version: String,
}

pub enum Stub {
    Missing,
    Current,
    Older(Version),
    Unversioned,
    Newer(Version),
}

impl Stub {
    fn of(contents: &[u8]) -> Self {
        let ours = crate::this_version();
        match stamped_version(&String::from_utf8_lossy(contents)) {
            None => Stub::Unversioned,
            Some(theirs) if theirs == ours => Stub::Current,
            Some(theirs) if theirs > ours => Stub::Newer(theirs),
            Some(theirs) => Stub::Older(theirs),
        }
    }
}

pub struct InstalledStub {
    pub path: PathBuf,
    pub stub: Stub,
    pub rewritten: bool,
}

pub fn installed_stub(fix: bool) -> Result<InstalledStub> {
    let path = stub_path()?;
    let mut file = match std::fs::OpenOptions::new().read(true).write(fix).open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Ok(InstalledStub { path, stub: Stub::Missing, rewritten: false });
        }
        Err(e) => return Err(e).with_context(|| format!("open {}", path.display())),
    };
    (if fix { file.lock() } else { file.lock_shared() }).with_context(|| format!("lock {}", path.display()))?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents).with_context(|| format!("read {}", path.display()))?;
    let stub = Stub::of(&contents);
    let rewritten = fix && matches!(stub, Stub::Older(_) | Stub::Unversioned);
    if rewritten {
        write_stub(&mut file, &path)?;
    }
    Ok(InstalledStub { path, stub, rewritten })
}

fn stub_path() -> Result<PathBuf> {
    let user =
        std::env::var_os("HOME").context("HOME is not set; it locates the agent skills folder ~/.claude/skills")?;
    Ok(PathBuf::from(user).join(".claude/skills/agent-terminal/SKILL.md"))
}

fn install_stub() -> Result<Installed> {
    let path = stub_path()?;
    let folder = path.parent().context("the stub has a folder")?;
    std::fs::create_dir_all(folder).with_context(|| format!("create {}", folder.display()))?;
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("open {}", path.display()))?;
    file.lock().with_context(|| format!("lock {}", path.display()))?;
    let mut existing = Vec::new();
    file.read_to_end(&mut existing).with_context(|| format!("read {}", path.display()))?;
    if let Stub::Newer(theirs) = Stub::of(&existing) {
        return Ok(Installed { installed: false, path, version: theirs.to_string() });
    }
    write_stub(&mut file, &path)?;
    Ok(Installed { installed: true, path, version: VERSION.to_string() })
}

fn write_stub(file: &mut std::fs::File, path: &Path) -> Result<()> {
    let stub = find("agent-terminal")?.guide();
    file.set_len(0).with_context(|| format!("truncate {}", path.display()))?;
    file.seek(SeekFrom::Start(0)).with_context(|| format!("rewind {}", path.display()))?;
    file.write_all(stub.as_bytes()).with_context(|| format!("write {}", path.display()))
}

fn stamped_version(stub: &str) -> Option<Version> {
    let (front, _) = stub.strip_prefix("---\n")?.split_once("\n---\n")?;
    let version = front
        .lines()
        .skip_while(|line| *line != "metadata:")
        .skip(1)
        .take_while(|line| line.starts_with(' '))
        .find_map(|line| line.trim_start().strip_prefix("version:"))?;
    Version::parse(version.trim().trim_matches('"')).ok()
}

fn find(name: &str) -> Result<&'static Skill> {
    SKILLS
        .iter()
        .find(|skill| skill.name == name)
        .ok_or_else(|| anyhow!("no skill named {name}; list them with `agent-terminal skills list`"))
}

fn list(json: bool) {
    let listed: Vec<&Skill> = SKILLS.iter().filter(|skill| !skill.is_stub()).collect();
    if json {
        let skills: Vec<_> = listed
            .iter()
            .map(|skill| serde_json::json!({ "name": skill.name, "description": skill.description() }))
            .collect();
        println!("{}", serde_json::json!({ "skills": skills }));
    } else {
        let width = listed.iter().map(|skill| skill.name.len()).max().unwrap_or(0);
        for skill in listed {
            println!("{:<width$}  {}", skill.name, skill.summary());
        }
    }
}

fn get(name: &str, full: bool, json: bool, cli: clap::Command) -> Result<()> {
    let skill = find(name)?;
    let mut content = skill.guide();
    if full {
        for (path, text) in &skill.shipped[1..] {
            content.push_str(&format!("\n--- {path} ---\n\n{text}"));
        }
        if skill.name == "core" {
            content.push_str(&format!("\n--- command reference ---\n\n{}", reference(cli)));
        }
    }
    if json {
        println!("{}", serde_json::json!({ "name": skill.name, "content": content }));
    } else {
        print!("{content}");
    }
    Ok(())
}

fn path(name: Option<&str>, json: bool) -> Result<()> {
    let skill = name.map(find).transpose()?;
    let cache = connection::home()?.join("skills");
    let root = cache.join(VERSION);
    for each in SKILLS {
        for (relative, text) in each.files() {
            write_atomically(&root.join(each.name).join(relative), &text)?;
        }
    }
    remove_older_versions(&cache)?;
    let path = skill.map_or_else(|| root.clone(), |skill| root.join(skill.name));
    if json {
        println!("{}", serde_json::json!({ "path": path }));
    } else {
        println!("{}", path.display());
    }
    Ok(())
}

fn remove_older_versions(cache: &Path) -> Result<()> {
    let ours = crate::this_version();
    for entry in std::fs::read_dir(cache).with_context(|| format!("read {}", cache.display()))? {
        let entry = entry.with_context(|| format!("read {}", cache.display()))?;
        let folder = entry.path();
        let older =
            entry.file_name().to_str().and_then(|name| Version::parse(name).ok()).is_some_and(|theirs| theirs < ours);
        if !older || !entry.file_type().with_context(|| format!("inspect {}", folder.display()))?.is_dir() {
            continue;
        }
        if let Err(e) = std::fs::remove_dir_all(&folder)
            && e.kind() != ErrorKind::NotFound
        {
            return Err(e).with_context(|| format!("remove {}", folder.display()));
        }
    }
    Ok(())
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
