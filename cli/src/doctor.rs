use std::fmt;
use std::io::{self, ErrorKind};
use std::os::unix::net::UnixStream;
use std::path::Path;

use serde::Serialize;

use crate::connection;
use crate::home::{home, socket_path};
use crate::native::{pty, render, shell};
use crate::output;
use crate::protocol::{RESTART_ADVICE, Shell};
use crate::skills::{self, Stub};
use crate::upgrade;
use crate::version::{BUILD, VERSION, daemon_of};

const OWN_THE_FOLDER: &str = "check that you own the .agent-terminal folder in your home folder and can write to it";

#[derive(Serialize)]
struct Check {
    name: &'static str,
    status: Status,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fix: Option<String>,
}

#[derive(Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Status {
    Ok,
    Problem,
    Fixed,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Problem => "problem",
            Status::Fixed => "fixed",
        }
    }
}

impl Check {
    fn ok(name: &'static str, message: impl Into<String>) -> Self {
        Self { name, status: Status::Ok, message: message.into(), fix: None }
    }

    fn fixed(name: &'static str, message: impl Into<String>) -> Self {
        Self { name, status: Status::Fixed, message: message.into(), fix: None }
    }

    fn problem(name: &'static str, message: impl Into<String>, fix: impl Into<String>) -> Self {
        Self { name, status: Status::Problem, message: message.into(), fix: Some(fix.into()) }
    }
}

#[derive(Serialize)]
struct Report<'a> {
    ok: bool,
    checks: &'a [Check],
}

pub(crate) fn run(fix: bool, json: bool) -> io::Result<i32> {
    let checks = [pty(), fonts(), shell(fix), daemon(fix), skill(fix)];
    let healthy = checks.iter().all(|check| check.status != Status::Problem);
    if json {
        output::print_json(&Report { ok: healthy, checks: &checks })?;
    } else {
        output::print(&fmt::from_fn(|f| checks.iter().try_for_each(|check| write_check(f, check))).to_string())?;
    }
    Ok(if healthy { 0 } else { crate::exit::ERROR })
}

fn write_check(f: &mut fmt::Formatter<'_>, check: &Check) -> fmt::Result {
    writeln!(f, "{:<9}{:<9}{}", check.status.as_str(), check.name, check.message)?;
    match &check.fix {
        Some(fix) => writeln!(f, "{:18}fix: {fix}", ""),
        None => Ok(()),
    }
}

fn pty() -> Check {
    match pty::check() {
        Ok(()) => Check::ok("pty", "opened a pseudo-terminal"),
        Err(e) => Check::problem(
            "pty",
            format!("{e:#}"),
            "check that /dev/ptmx exists and that you can read and write it (in a container, mount devpts)",
        ),
    }
}

fn fonts() -> Check {
    match render::check_fonts() {
        Ok(fonts) => Check::ok("fonts", format!("{} are built in", listed(&fonts))),
        Err(missing) => Check::problem(
            "fonts",
            format!("the built-in {missing} did not load, so screenshots show □ for what it draws"),
            format!("reinstall agent-terminal (npm i -g {}, or brew reinstall {})", upgrade::PACKAGE, upgrade::FORMULA),
        ),
    }
}

fn listed(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [only] => only.to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn shell(fix: bool) -> Check {
    let audited = home().and_then(|home| {
        let scripts = shell::Scripts { home: &home };
        if fix { scripts.repair() } else { scripts.audit() }
    });
    match audited {
        Ok(audit) => audited_shell(&audit, fix),
        Err(e) => Check::problem("shell", format!("{e:#}"), OWN_THE_FOLDER),
    }
}

fn audited_shell(audit: &shell::Audit, fix: bool) -> Check {
    let (changed, left_over) = (audit.outdated.len(), audit.leftovers.len());
    if changed + left_over > 0 && !fix {
        return Check::problem(
            "shell",
            format!(
                "the shell integration scripts in {} are not this version's ({changed} changed, {left_over} left over)",
                audit.folder.display()
            ),
            "agent-terminal doctor --fix brings them up to date",
        );
    }
    match your_shell() {
        Ok(yours) => scripts_check(audit, yours.name()),
        Err(message) => {
            Check::problem("shell", message, "open a shell with agent-terminal shell --shell zsh (or bash, fish)")
        }
    }
}

fn scripts_check(audit: &shell::Audit, yours: &str) -> Check {
    let folder = audit.folder.display();
    let (changed, left_over) = (audit.outdated.len(), audit.leftovers.len());
    if changed + left_over > 0 {
        return Check::fixed(
            "shell",
            format!(
                "brought the shell integration scripts in {folder} up to date (rewrote {changed}, removed {left_over})"
            ),
        );
    }
    if audit.current == 0 {
        return Check::ok(
            "shell",
            format!(
                "{yours} (your SHELL) has shell integration; `agent-terminal shell` writes its scripts when it first runs"
            ),
        );
    }
    Check::ok("shell", format!("{yours} (your SHELL) has shell integration; the scripts in {folder} are up to date"))
}

fn your_shell() -> Result<Shell, String> {
    let program = std::env::var_os("SHELL")
        .ok_or("SHELL is not set, so `agent-terminal shell` does not know which shell to open")?;
    let program = program.to_string_lossy();
    Shell::of_program(&program)
        .ok_or_else(|| format!("SHELL is {program}, which has no shell integration (zsh, bash and fish have it)"))
}

fn daemon(fix: bool) -> Check {
    match socket_path() {
        Ok(socket) => reach(&socket, fix),
        Err(e) => {
            Check::problem("daemon", format!("{e:#}"), "give your user account a home folder in the user database")
        }
    }
}

fn reach(socket: &Path, fix: bool) -> Check {
    match UnixStream::connect(socket) {
        Ok(_) => running(socket),
        Err(e) => unreachable(socket, &e, fix),
    }
}

fn unreachable(socket: &Path, e: &io::Error, fix: bool) -> Check {
    match e.kind() {
        ErrorKind::NotFound => Check::ok("daemon", "not running; the next command starts it"),
        ErrorKind::ConnectionRefused if fix => remove_stale(socket),
        ErrorKind::ConnectionRefused => Check::problem(
            "daemon",
            format!("stale socket {}: no daemon is listening on it", socket.display()),
            "agent-terminal doctor --fix removes it",
        ),
        _ => Check::problem(
            "daemon",
            format!("cannot reach the daemon socket {}: {e}", socket.display()),
            "the daemon's folder must be one you own, with a path short enough for a socket",
        ),
    }
}

fn remove_stale(socket: &Path) -> Check {
    match connection::remove_stale_socket() {
        Ok(true) => Check::fixed("daemon", format!("removed the stale socket {}", socket.display())),
        Ok(false) => daemon(false),
        Err(e) => Check::problem(
            "daemon",
            format!("cannot remove the stale socket {}: {e:#}", socket.display()),
            format!("remove it by hand: rm {}", socket.display()),
        ),
    }
}

fn running(socket: &Path) -> Check {
    let socket = socket.display();
    let running = match connection::started_build() {
        Ok(running) => running,
        Err(e) => return Check::problem("daemon", format!("{e:#}"), OWN_THE_FOLDER),
    };
    let daemon = daemon_of(running.as_deref());
    if running.as_deref() == Some(BUILD) {
        return Check::ok("daemon", format!("running {daemon}, listening on {socket}"));
    }
    Check::problem(
        "daemon",
        format!("running {daemon}, not {BUILD} like this command, listening on {socket}"),
        format!("the next command replaces it if it has no open terminals; otherwise {RESTART_ADVICE}"),
    )
}

fn skill(fix: bool) -> Check {
    let checked = if fix { skills::repair_stub() } else { skills::installed_stub() };
    let installed = match checked {
        Ok(installed) => installed,
        Err(e) => {
            return Check::problem(
                "skill",
                format!("{e:#}"),
                "check that HOME is set and that you can read and write ~/.claude/skills/agent-terminal/SKILL.md",
            );
        }
    };
    let path = installed.path.display();
    if installed.rewritten {
        return Check::fixed(
            "skill",
            format!("rewrote the agent-terminal skill at {path} as this version ({VERSION})"),
        );
    }
    stub_check(installed.stub, &path.to_string())
}

fn stub_check(stub: Stub, path: &str) -> Check {
    let rewrite = "agent-terminal doctor --fix rewrites it";
    match stub {
        Stub::Missing => Check::ok(
            "skill",
            format!(
                "the agent-terminal skill is not installed at {path}; `agent-terminal install --skills` installs it"
            ),
        ),
        Stub::Current => Check::ok("skill", format!("the agent-terminal skill at {path} is this version ({VERSION})")),
        Stub::Older(theirs) => Check::problem(
            "skill",
            format!("the agent-terminal skill at {path} is {theirs}, older than this version ({VERSION})"),
            rewrite,
        ),
        Stub::Unversioned => Check::problem(
            "skill",
            format!("the agent-terminal skill at {path} has no version, so it may not be this version ({VERSION})"),
            rewrite,
        ),
        Stub::Newer(theirs) => Check::problem(
            "skill",
            format!("the agent-terminal skill at {path} is {theirs}, newer than this version ({VERSION})"),
            "agent-terminal upgrade brings agent-terminal up to it (doctor --fix does not downgrade a skill)",
        ),
    }
}
