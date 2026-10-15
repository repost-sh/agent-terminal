use std::ffi::OsStr;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};

use semver::Version;
use serde::Deserialize;

use crate::VERSION;
use crate::output;
use crate::protocol::ErrorBody;
use crate::skills::{self, Installed, Stub};

const PACKAGE: &str = "@repost/agent-terminal";
const FORMULA: &str = "repost-sh/tap/agent-terminal";
const NPM_NESTING: [&str; 4] = ["node_modules", "@repost", "agent-terminal", "node_modules"];

struct Installer {
    name: &'static str,
    program: &'static str,
    source: &'static str,
    refresh: Option<&'static [&'static str]>,
    latest: &'static [&'static str],
    latest_in: fn(&str) -> Option<String>,
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
    latest_in: printed_version,
    reinstall: &["install", "-g", "@repost/agent-terminal@latest"],
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
    latest_in: printed_version,
    reinstall: &["add", "-g", "@repost/agent-terminal@latest"],
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
    latest_in: printed_version,
    reinstall: &["global", "add", "@repost/agent-terminal@latest"],
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
    latest_in: printed_version,
    reinstall: &["add", "-g", "@repost/agent-terminal@latest"],
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
    latest_in: formula_version,
    reinstall: &["upgrade", FORMULA],
    listing: None,
    folder: &["--prefix", FORMULA],
    entry: "bin/agent-terminal",
};

const EVERY_INSTALLER: [&Installer; 5] = [&NPM, &PNPM, &YARN, &BUN, &HOMEBREW];

fn printed_version(printed: &str) -> Option<String> {
    Some(printed.trim().to_string())
}

fn formula_version(printed: &str) -> Option<String> {
    let info: serde_json::Value = serde_json::from_str(printed).ok()?;
    info["formulae"][0]["versions"]["stable"].as_str().map(str::to_string)
}

impl Installer {
    fn of(binary: &Path) -> Result<&'static Self, String> {
        let parts: Vec<&OsStr> = binary.components().map(|part| part.as_os_str()).collect();
        let has = |run: &[&str]| parts.windows(run.len()).any(|window| window == run);
        let pnpm_store_links = parts.windows(2).any(|window| {
            window[1] == "links"
                && window[0]
                    .to_str()
                    .and_then(|name| name.strip_prefix('v'))
                    .is_some_and(|number| !number.is_empty() && number.bytes().all(|digit| digit.is_ascii_digit()))
        });
        if has(&["Cellar"]) {
            return Ok(&HOMEBREW);
        }
        if pnpm_store_links || has(&["node_modules", ".pnpm"]) {
            return Ok(&PNPM);
        }
        if has(&["yarn", "global", "node_modules"]) {
            return Ok(&YARN);
        }
        if has(&["install", "global", "node_modules"]) {
            return Ok(&BUN);
        }
        if has(&NPM_NESTING) {
            return Ok(&NPM);
        }
        if !has(&["node_modules"]) {
            return Err(undetectable(binary, &[]));
        }
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
        match (&listed[..], &failed[..]) {
            ([installer], []) => Ok(installer),
            _ => Err(undetectable(binary, &failed)),
        }
    }

    fn lists_agent_terminal(&self, listing: &[&str]) -> Result<bool, String> {
        let listed = match self.output(listing, Stdio::piped()) {
            Ok(listed) => listed,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(self.cannot_run(listing, &e)),
        };
        if !listed.status.success() {
            return Err(format!("`{}` failed ({})", self.line(listing), listed.status));
        }
        let listed = String::from_utf8_lossy(&listed.stdout);
        Ok(listed
            .match_indices(PACKAGE)
            .any(|(at, _)| matches!(listed[at + PACKAGE.len()..].chars().next(), Some('@' | ' '))))
    }

    fn line(&self, args: &[&str]) -> String {
        format!("{} {}", self.program, args.join(" "))
    }

    fn latest(&self) -> Result<Version, String> {
        if let Some(refresh) = self.refresh {
            let refreshed = self.run(refresh, std::io::stderr().into())?;
            if !refreshed.status.success() {
                return Err(self.unreachable(refresh, refreshed.status));
            }
        }
        let answer = self.run(self.latest, Stdio::piped())?;
        if !answer.status.success() {
            return Err(self.unreachable(self.latest, answer.status));
        }
        let printed = String::from_utf8_lossy(&answer.stdout);
        (self.latest_in)(&printed)
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
        self.succeed(self.reinstall, std::io::stderr().into()).map(drop)
    }

    fn entry_point(&self) -> Result<PathBuf, String> {
        let answer = self.succeed(self.folder, Stdio::piped())?;
        let printed = String::from_utf8_lossy(&answer.stdout);
        match printed.trim() {
            "" => Err(format!("`{}` printed no folder", self.line(self.folder))),
            folder => Ok(Path::new(folder).join(self.entry)),
        }
    }

    fn succeed(&self, args: &[&str], stdout: Stdio) -> Result<Output, String> {
        let output = self.run(args, stdout)?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(format!("`{}` failed ({}); see its output above", self.line(args), output.status))
        }
    }

    fn run(&self, args: &[&str], stdout: Stdio) -> Result<Output, String> {
        self.output(args, stdout).map_err(|e| self.cannot_run(args, &e))
    }

    fn output(&self, args: &[&str], stdout: Stdio) -> std::io::Result<Output> {
        Command::new(self.program).args(args).stdin(Stdio::null()).stdout(stdout).stderr(Stdio::inherit()).output()
    }

    fn cannot_run(&self, args: &[&str], error: &std::io::Error) -> String {
        format!("cannot run `{}`: {error}", self.line(args))
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

pub fn run(json: bool) -> i32 {
    match upgrade(json) {
        Ok(()) => 0,
        Err(message) => output::refuse(&message, json),
    }
}

fn upgrade(json: bool) -> Result<(), String> {
    let invoked = std::env::current_exe().map_err(|e| format!("cannot find the agent-terminal binary: {e}"))?;
    let binary = invoked.canonicalize().map_err(|e| format!("cannot resolve {}: {e}", invoked.display()))?;
    let installer = Installer::of(&binary)?;
    let latest = installer.latest()?;
    let from = VERSION;
    let mut report = serde_json::json!({
        "installed_with": installer.name,
        "from": from,
        "latest": latest.to_string(),
        "to": from,
        "stub_refreshed": false,
    });
    if latest == crate::this_version() {
        if json {
            println!("{report}");
        } else {
            println!("agent-terminal {from} is already the latest version");
        }
        return Ok(());
    }

    installer.reinstall()?;
    let entry = installer.entry_point();
    let to = entry.clone().and_then(|entry| installed_version(&entry));
    let stub = refresh_stub(&entry);
    let mut text = match &to {
        Ok(to) => {
            report["to"] = serde_json::json!(to);
            format!("upgraded agent-terminal {from} to {to} with {}", installer.name)
        }
        Err(reason) => {
            report["to"] = serde_json::Value::Null;
            report["version_unknown"] = serde_json::json!(reason);
            format!(
                "upgraded agent-terminal {from} with {}, but cannot tell the version now installed ({reason}); run `agent-terminal version`",
                installer.name
            )
        }
    };
    match &stub {
        Ok(None) => {}
        Ok(Some(refreshed)) => {
            report["stub_refreshed"] = serde_json::json!(refreshed.installed);
            if refreshed.installed {
                text.push_str(&format!(
                    "\nrefreshed the agent-terminal skill {} at {}",
                    refreshed.version,
                    refreshed.path.display()
                ));
            }
        }
        Err(reason) => {
            report["stub_error"] = serde_json::json!(reason);
            text.push_str(&format!(
                "\ncannot refresh the agent-terminal skill ({reason}); run `agent-terminal install --skills`"
            ));
        }
    }
    if json {
        println!("{report}");
    } else {
        println!("{text}");
    }
    Ok(())
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

#[derive(Deserialize)]
struct Refused {
    error: ErrorBody,
}

fn refresh_stub(entry: &Result<PathBuf, String>) -> Result<Option<Installed>, String> {
    let installed = skills::installed_stub().map_err(|e| format!("{e:#}"))?;
    if let Stub::Missing = installed.stub {
        return Ok(None);
    }
    let binary = entry.as_ref().map_err(String::clone)?;
    let args = ["--json", "install", "--skills"];
    let line = format!("{} {}", binary.display(), args.join(" "));
    let output = Command::new(binary)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("cannot run `{line}`: {e}"))?;
    if output.status.success() {
        return serde_json::from_slice(&output.stdout)
            .map(Some)
            .map_err(|e| format!("`{line}` printed {:?}: {e}", String::from_utf8_lossy(&output.stdout).trim()));
    }
    Err(serde_json::from_slice::<Refused>(&output.stdout)
        .map_or_else(|_| format!("`{line}` failed ({})", output.status), |refused| refused.error.message))
}
