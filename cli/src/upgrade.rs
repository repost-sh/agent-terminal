use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, ExitStatus, Output, Stdio};

use semver::Version;
use serde::Deserialize;

use crate::output;
use crate::protocol::ErrorBody;
use crate::skills::{self, Installed, Stub};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const PACKAGE: &str = "@repost/agent-terminal";
const NPM_NESTING: [&str; 4] = ["node_modules", "@repost", "agent-terminal", "node_modules"];

#[derive(Clone, Copy)]
enum Installer {
    Npm,
    Pnpm,
    Yarn,
    Bun,
    Homebrew,
}

const EVERY_INSTALLER: [Installer; 5] =
    [Installer::Npm, Installer::Pnpm, Installer::Yarn, Installer::Bun, Installer::Homebrew];

const GLOBAL_LISTINGS: [(Installer, &[&str]); 4] = [
    (Installer::Npm, &["ls", "-g", "--depth=0"]),
    (Installer::Pnpm, &["ls", "-g", "--depth=0"]),
    (Installer::Yarn, &["global", "list"]),
    (Installer::Bun, &["pm", "ls", "-g"]),
];

impl Installer {
    fn of(binary: &Path) -> Option<Self> {
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
            return Some(Installer::Homebrew);
        }
        if pnpm_store_links || has(&["node_modules", ".pnpm"]) {
            return Some(Installer::Pnpm);
        }
        if has(&["yarn", "global", "node_modules"]) {
            return Some(Installer::Yarn);
        }
        if has(&["install", "global", "node_modules"]) {
            return Some(Installer::Bun);
        }
        if has(&NPM_NESTING) {
            return Some(Installer::Npm);
        }
        if !has(&["node_modules"]) {
            return None;
        }
        let listed: Vec<Installer> = GLOBAL_LISTINGS
            .into_iter()
            .filter(|(installer, args)| lists_agent_terminal(installer.program(), args))
            .map(|(installer, _)| installer)
            .collect();
        match listed[..] {
            [installer] => Some(installer),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Installer::Npm => "npm",
            Installer::Pnpm => "pnpm",
            Installer::Yarn => "yarn",
            Installer::Bun => "bun",
            Installer::Homebrew => "Homebrew",
        }
    }

    fn program(self) -> &'static str {
        match self {
            Installer::Npm => "npm",
            Installer::Pnpm => "pnpm",
            Installer::Yarn => "yarn",
            Installer::Bun => "bun",
            Installer::Homebrew => "brew",
        }
    }

    fn reinstall_args(self) -> &'static [&'static str] {
        match self {
            Installer::Npm => &["install", "-g", "@repost/agent-terminal@latest"],
            Installer::Pnpm | Installer::Bun => &["add", "-g", "@repost/agent-terminal@latest"],
            Installer::Yarn => &["global", "add", "@repost/agent-terminal@latest"],
            Installer::Homebrew => &["upgrade", "repost-sh/tap/agent-terminal"],
        }
    }

    fn latest_query(self) -> &'static [&'static str] {
        match self {
            Installer::Npm | Installer::Pnpm => &["view", PACKAGE, "version"],
            Installer::Yarn => &["info", PACKAGE, "version", "--silent"],
            Installer::Bun => &["pm", "view", PACKAGE, "version"],
            Installer::Homebrew => &["info", "--json=v2", "repost-sh/tap/agent-terminal"],
        }
    }

    fn line(self, args: &[&str]) -> String {
        format!("{} {}", self.program(), args.join(" "))
    }

    fn latest(self) -> Result<Version, String> {
        if let Installer::Homebrew = self {
            let update = ["update"];
            let updated = self.run(&update, std::io::stderr().into())?;
            if !updated.status.success() {
                return Err(self.unreachable(&update, updated.status));
            }
        }
        let query = self.latest_query();
        let answer = self.run(query, Stdio::piped())?;
        if !answer.status.success() {
            return Err(self.unreachable(query, answer.status));
        }
        let printed = String::from_utf8_lossy(&answer.stdout);
        let latest = match self {
            Installer::Homebrew => serde_json::from_str::<serde_json::Value>(&printed)
                .ok()
                .and_then(|info| info["formulae"][0]["versions"]["stable"].as_str().map(str::to_string)),
            _ => Some(printed.trim().to_string()),
        };
        latest
            .and_then(|latest| Version::parse(&latest).ok())
            .ok_or_else(|| format!("`{}` printed {:?}, not a version", self.line(query), printed.trim()))
    }

    fn unreachable(self, args: &[&str], status: ExitStatus) -> String {
        let source = match self {
            Installer::Homebrew => "the Homebrew tap",
            _ => "the registry",
        };
        format!(
            "cannot reach {source} to find the latest version: `{}` failed ({status}); see its output above",
            self.line(args)
        )
    }

    fn reinstall(self) -> Result<(), String> {
        let args = self.reinstall_args();
        let reinstalled = self.run(args, std::io::stderr().into())?;
        if reinstalled.status.success() {
            Ok(())
        } else {
            Err(format!("`{}` failed ({}); see its output above", self.line(args), reinstalled.status))
        }
    }

    fn run(self, args: &[&str], stdout: Stdio) -> Result<Output, String> {
        Command::new(self.program())
            .args(args)
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| format!("cannot run `{}`: {e}", self.line(args)))
    }
}

fn lists_agent_terminal(program: &str, args: &[&str]) -> bool {
    Command::new(program).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().is_ok_and(|listing| {
        let listed = String::from_utf8_lossy(&listing.stdout);
        listing.status.success()
            && listed
                .match_indices(PACKAGE)
                .any(|(at, _)| matches!(listed[at + PACKAGE.len()..].chars().next(), Some('@' | ' ')))
    })
}

fn undetectable(binary: &Path) -> String {
    let commands: Vec<String> =
        EVERY_INSTALLER.iter().map(|installer| format!("  {}", installer.line(installer.reinstall_args()))).collect();
    format!(
        "cannot tell which installer put agent-terminal at {}, so upgrade cannot update it; run the command of the one you used:\n{}",
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
    let installer = Installer::of(&binary).ok_or_else(|| undetectable(&binary))?;
    let latest = installer.latest()?;
    let from = VERSION;
    let mut report = serde_json::json!({
        "installed_with": installer.name(),
        "from": from,
        "latest": latest.to_string(),
        "to": from,
        "stub_refreshed": false,
    });
    if latest == Version::parse(from).expect("the package version is semver") {
        if json {
            println!("{report}");
        } else {
            println!("agent-terminal {from} is already the latest version");
        }
        return Ok(());
    }

    installer.reinstall()?;
    let to = installed_version(&invoked);
    let stub = refresh_stub(&invoked);
    let mut text = match &to {
        Ok(to) => {
            report["to"] = serde_json::json!(to);
            format!("upgraded agent-terminal {from} to {to} with {}", installer.name())
        }
        Err(reason) => {
            report["to"] = serde_json::Value::Null;
            report["version_unknown"] = serde_json::json!(reason);
            format!(
                "upgraded agent-terminal {from} with {}, but cannot tell the version now installed ({reason}); run `agent-terminal version`",
                installer.name()
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

fn refresh_stub(binary: &Path) -> Result<Option<Installed>, String> {
    let installed = skills::installed_stub(false).map_err(|e| format!("{e:#}"))?;
    if let Stub::Missing = installed.stub {
        return Ok(None);
    }
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
