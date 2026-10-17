use std::ffi::OsStr;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};

use std::io;
use std::path::Component;

use semver::Version;
use serde::Serialize;

use crate::output;
use crate::version::VERSION;

const PACKAGE: &str = "@repost/agent-terminal";
const FORMULA: &str = "repost-sh/tap/agent-terminal";
const LATEST: &str = "@repost/agent-terminal@latest";
const NPM_NESTING: [&str; 4] = ["node_modules", "@repost", "agent-terminal", "node_modules"];

struct Installer {
    name: &'static str,
    program: &'static str,
    source: &'static str,
    refresh: Option<&'static [&'static str]>,
    latest: &'static [&'static str],
    latest_in: Answer,
    reinstall: &'static [&'static str],
    listing: Option<&'static [&'static str]>,
    folder: &'static [&'static str],
    entry: &'static str,
}

const NPM: Installer = Installer {
    name: "npm",
    program: "npm",
    source: "the registry",
    refresh: None,
    latest: &["view", PACKAGE, "version"],
    latest_in: Answer::Printed,
    reinstall: &["install", "-g", LATEST],
    listing: Some(&["ls", "-g", "--depth=0"]),
    folder: &["prefix", "-g"],
    entry: "bin/agent-terminal",
};

const PNPM: Installer = Installer {
    name: "pnpm",
    program: "pnpm",
    source: "the registry",
    refresh: None,
    latest: &["view", PACKAGE, "version"],
    latest_in: Answer::Printed,
    reinstall: &["add", "-g", LATEST],
    listing: Some(&["ls", "-g", "--depth=0"]),
    folder: &["bin", "-g"],
    entry: "agent-terminal",
};

const YARN: Installer = Installer {
    name: "yarn",
    program: "yarn",
    source: "the registry",
    refresh: None,
    latest: &["info", PACKAGE, "version", "--silent"],
    latest_in: Answer::Printed,
    reinstall: &["global", "add", LATEST],
    listing: Some(&["global", "list"]),
    folder: &["global", "bin"],
    entry: "agent-terminal",
};

const BUN: Installer = Installer {
    name: "bun",
    program: "bun",
    source: "the registry",
    refresh: None,
    latest: &["pm", "view", PACKAGE, "version"],
    latest_in: Answer::Printed,
    reinstall: &["add", "-g", LATEST],
    listing: Some(&["pm", "ls", "-g"]),
    folder: &["pm", "bin", "-g"],
    entry: "agent-terminal",
};

const HOMEBREW: Installer = Installer {
    name: "Homebrew",
    program: "brew",
    source: "the Homebrew tap",
    refresh: Some(&["update"]),
    latest: &["info", "--json=v2", FORMULA],
    latest_in: Answer::FormulaInfo,
    reinstall: &["upgrade", FORMULA],
    listing: None,
    folder: &["--prefix", FORMULA],
    entry: "bin/agent-terminal",
};

const EVERY_INSTALLER: [&Installer; 5] = [&NPM, &PNPM, &YARN, &BUN, &HOMEBREW];

#[derive(Clone, Copy)]
enum Answer {
    Printed,
    FormulaInfo,
}

impl Answer {
    fn version(self, printed: &str) -> Option<String> {
        match self {
            Answer::Printed => Some(printed.trim().to_string()),
            Answer::FormulaInfo => {
                let info: serde_json::Value = serde_json::from_str(printed).ok()?;
                info.pointer("/formulae/0/versions/stable")?.as_str().map(str::to_string)
            }
        }
    }
}

impl Installer {
    fn of(binary: &Path) -> Result<&'static Self, String> {
        let parts: Vec<&OsStr> = binary.components().map(Component::as_os_str).collect();
        if let Some(installer) = by_path(&parts) {
            return Ok(installer);
        }
        if !contains_run(&parts, &["node_modules"]) {
            return Err(undetectable(binary, &[]));
        }
        by_listing(binary)
    }

    fn lists_agent_terminal(&self, listing: &[&str]) -> Result<bool, String> {
        let listed = match self.spawn(listing, Stdio::piped()) {
            Ok(listed) => listed,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(self.cannot_run(listing, &e)),
        };
        if !listed.status.success() {
            return Err(format!("`{}` failed ({})", self.line(listing), listed.status));
        }
        let listed = String::from_utf8_lossy(&listed.stdout);
        Ok(listed.match_indices(PACKAGE).any(|(at, _)| {
            matches!(listed.get(at + PACKAGE.len()..).and_then(|after| after.chars().next()), Some('@' | ' '))
        }))
    }

    fn line(&self, args: &[&str]) -> String {
        format!("{} {}", self.program, args.join(" "))
    }

    fn latest(&self) -> Result<Version, String> {
        if let Some(refresh) = self.refresh {
            self.run(refresh, io::stderr().into(), |status| self.unreachable(refresh, status))?;
        }
        let answer = self.run(self.latest, Stdio::piped(), |status| self.unreachable(self.latest, status))?;
        let printed = String::from_utf8_lossy(&answer.stdout);
        self.latest_in
            .version(&printed)
            .and_then(|latest| Version::parse(&latest).ok())
            .ok_or_else(|| format!("`{}` printed {:?}, not a version", self.line(self.latest), printed.trim()))
    }

    fn unreachable(&self, args: &[&str], status: ExitStatus) -> String {
        format!(
            "cannot reach {} to find the latest version: `{}` failed ({status}); see its output above",
            self.source,
            self.line(args)
        )
    }

    fn reinstall(&self) -> Result<(), String> {
        self.run(self.reinstall, io::stderr().into(), |status| self.failed(self.reinstall, status)).map(drop)
    }

    fn entry_point(&self) -> Result<PathBuf, String> {
        let answer = self.run(self.folder, Stdio::piped(), |status| self.failed(self.folder, status))?;
        let printed = String::from_utf8_lossy(&answer.stdout);
        match printed.trim() {
            "" => Err(format!("`{}` printed no folder", self.line(self.folder))),
            folder => Ok(Path::new(folder).join(self.entry)),
        }
    }

    fn run(&self, args: &[&str], stdout: Stdio, failed: impl FnOnce(ExitStatus) -> String) -> Result<Output, String> {
        let output = self.spawn(args, stdout).map_err(|e| self.cannot_run(args, &e))?;
        if output.status.success() { Ok(output) } else { Err(failed(output.status)) }
    }

    fn failed(&self, args: &[&str], status: ExitStatus) -> String {
        format!("`{}` failed ({status}); see its output above", self.line(args))
    }

    fn spawn(&self, args: &[&str], stdout: Stdio) -> io::Result<Output> {
        Command::new(self.program).args(args).stdin(Stdio::null()).stdout(stdout).stderr(Stdio::inherit()).output()
    }

    fn cannot_run(&self, args: &[&str], error: &io::Error) -> String {
        format!("cannot run `{}`: {error}", self.line(args))
    }
}

fn by_path(parts: &[&OsStr]) -> Option<&'static Installer> {
    let has = |run: &[&str]| contains_run(parts, run);
    if has(&["Cellar"]) {
        return Some(&HOMEBREW);
    }
    if pnpm_store_links(parts) || has(&["node_modules", ".pnpm"]) {
        return Some(&PNPM);
    }
    if has(&["yarn", "global", "node_modules"]) {
        return Some(&YARN);
    }
    if has(&["install", "global", "node_modules"]) {
        return Some(&BUN);
    }
    has(&NPM_NESTING).then_some(&NPM)
}

fn contains_run(parts: &[&OsStr], run: &[&str]) -> bool {
    parts.windows(run.len()).any(|window| window == run)
}

fn pnpm_store_links(parts: &[&OsStr]) -> bool {
    parts.iter().zip(parts.iter().skip(1)).any(|(version, links)| {
        *links == "links"
            && version
                .to_str()
                .and_then(|name| name.strip_prefix('v'))
                .is_some_and(|number| !number.is_empty() && number.bytes().all(|digit| digit.is_ascii_digit()))
    })
}

fn by_listing(binary: &Path) -> Result<&'static Installer, String> {
    let mut listed = Vec::new();
    let mut failed = Vec::new();
    for installer in EVERY_INSTALLER {
        let Some(listing) = installer.listing else {
            continue;
        };
        match installer.lists_agent_terminal(listing) {
            Ok(true) => listed.push(installer),
            Ok(false) => {}
            Err(reason) => failed.push(reason),
        }
    }
    match (listed.as_slice(), failed.as_slice()) {
        ([installer], []) => Ok(installer),
        _ => Err(undetectable(binary, &failed)),
    }
}

fn undetectable(binary: &Path, failed: &[String]) -> String {
    let commands: Vec<String> =
        EVERY_INSTALLER.iter().map(|installer| format!("  {}", installer.line(installer.reinstall))).collect();
    let because = if failed.is_empty() { String::new() } else { format!(" ({})", failed.join("; ")) };
    format!(
        "cannot tell which installer put agent-terminal at {}{because}, so upgrade cannot update it; run the command of the one you used:\n{}",
        binary.display(),
        commands.join("\n")
    )
}

pub(crate) fn run(json: bool) -> io::Result<i32> {
    match upgrade(json) {
        Ok(text) => output::print(&text).map(|()| 0),
        Err(message) => output::refuse(&message, json),
    }
}

#[derive(Serialize)]
struct Upgraded {
    installed_with: &'static str,
    from: &'static str,
    latest: String,
    to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version_unknown: Option<String>,
}

fn upgrade(json: bool) -> Result<String, String> {
    let invoked = std::env::current_exe().map_err(|e| format!("cannot find the agent-terminal binary: {e}"))?;
    let binary = invoked.canonicalize().map_err(|e| format!("cannot resolve {}: {e}", invoked.display()))?;
    let installer = Installer::of(&binary)?;
    let latest = installer.latest()?;
    let mut report = Upgraded {
        installed_with: installer.name,
        from: VERSION,
        latest: latest.to_string(),
        to: Some(VERSION.to_string()),
        version_unknown: None,
    };
    let ours = crate::version::this_version().map_err(|e| format!("this agent-terminal's version {VERSION}: {e}"))?;
    if latest == ours {
        return printed(json, &report, &format!("agent-terminal {VERSION} is already the latest version"));
    }
    installer.reinstall()?;
    let entry = installer.entry_point();
    let upgraded = report.upgraded(installer, entry.as_deref().map_err(String::clone));
    printed(json, &report, &upgraded)
}

impl Upgraded {
    fn upgraded(&mut self, installer: &Installer, new_binary: Result<&Path, String>) -> String {
        match new_binary.and_then(installed_version) {
            Ok(to) => {
                let text = format!("upgraded agent-terminal {VERSION} to {to} with {}", installer.name);
                self.to = Some(to);
                text
            }
            Err(reason) => {
                let text = format!(
                    "upgraded agent-terminal {VERSION} with {}, but cannot tell the version now installed ({reason}); run `agent-terminal version`",
                    installer.name
                );
                self.to = None;
                self.version_unknown = Some(reason);
                text
            }
        }
    }
}

fn printed(json: bool, report: &Upgraded, text: &str) -> Result<String, String> {
    if !json {
        return Ok(format!("{text}\n"));
    }
    let report = serde_json::to_value(report).map_err(|e| format!("encode the upgrade report: {e}"))?;
    Ok(format!("{report}\n"))
}

fn installed_version(binary: &Path) -> Result<String, String> {
    let output =
        Command::new(binary).arg("--version").output().map_err(|e| format!("run {}: {e}", binary.display()))?;
    let printed = String::from_utf8_lossy(&output.stdout);
    printed
        .trim()
        .strip_prefix("agent-terminal ")
        .map(str::to_string)
        .ok_or_else(|| format!("{} --version printed {:?}", binary.display(), printed.trim()))
}
