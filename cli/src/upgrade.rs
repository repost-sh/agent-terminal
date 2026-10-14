use std::path::Path;
use std::process::{Command, Stdio};

use crate::output;

#[derive(Clone, Copy)]
enum Installer {
    Npm,
    Homebrew,
}

impl Installer {
    fn of(binary: &Path) -> Option<Self> {
        binary.components().find_map(|part| match part.as_os_str().to_str() {
            Some("node_modules") => Some(Installer::Npm),
            Some("Cellar") => Some(Installer::Homebrew),
            _ => None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Installer::Npm => "npm",
            Installer::Homebrew => "Homebrew",
        }
    }

    fn command(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Installer::Npm => ("npm", &["install", "-g", "agent-terminal@latest"]),
            Installer::Homebrew => ("brew", &["upgrade", "agent-terminal"]),
        }
    }
}

pub fn run(json: bool) -> i32 {
    let invoked = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => return output::fail(&format!("cannot find the agent-terminal binary: {e}"), crate::exit::ERROR),
    };
    let binary = match invoked.canonicalize() {
        Ok(path) => path,
        Err(e) => return output::fail(&format!("cannot resolve {}: {e}", invoked.display()), crate::exit::ERROR),
    };
    let Some(installer) = Installer::of(&binary) else {
        return output::fail(
            &format!(
                "agent-terminal at {} was not installed with npm or Homebrew, so upgrade cannot update it; install it with `npm i -g agent-terminal`, or update it the way you installed it",
                binary.display()
            ),
            crate::exit::ERROR,
        );
    };

    let (program, args) = installer.command();
    let line = format!("{program} {}", args.join(" "));
    match Command::new(program).args(args).stdin(Stdio::null()).stdout(std::io::stderr()).status() {
        Ok(status) if status.success() => {}
        Ok(status) => {
            return output::fail(&format!("`{line}` failed ({status}); see its output above"), crate::exit::ERROR);
        }
        Err(e) => return output::fail(&format!("cannot run `{line}`: {e}"), crate::exit::ERROR),
    }

    let from = env!("CARGO_PKG_VERSION");
    let to = installed_version(&invoked);
    if json {
        let mut upgraded = serde_json::json!({ "installed_with": installer.name(), "from": from, "to": null });
        match &to {
            Ok(to) => upgraded["to"] = serde_json::json!(to),
            Err(reason) => upgraded["version_unknown"] = serde_json::json!(reason),
        }
        println!("{upgraded}");
    } else {
        match &to {
            Ok(to) if to == from => println!("agent-terminal {from} is already the latest version"),
            Ok(to) => println!("upgraded agent-terminal {from} to {to} with {}", installer.name()),
            Err(reason) => println!(
                "upgraded agent-terminal {from} with {}, but cannot tell the version now installed ({reason}); run `agent-terminal version`",
                installer.name()
            ),
        }
    }
    0
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
