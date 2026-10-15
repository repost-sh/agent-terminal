use std::io::ErrorKind;
use std::os::unix::net::UnixStream;

use serde::Serialize;

use crate::connection;
use crate::native::{pty, render, shell};
use crate::protocol::Shell;

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

pub fn run(fix: bool, json: bool) -> i32 {
    let checks = [pty(), fonts(), shell(fix), daemon(fix)];
    let healthy = checks.iter().all(|check| check.status != Status::Problem);
    if json {
        println!("{}", serde_json::json!({ "ok": healthy, "checks": checks }));
    } else {
        for check in &checks {
            println!("{:<9}{:<9}{}", check.status.as_str(), check.name, check.message);
            if let Some(fix) = &check.fix {
                println!("{:18}fix: {fix}", "");
            }
        }
    }
    if healthy { 0 } else { crate::exit::ERROR }
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
            "reinstall agent-terminal (npm i -g @repost/agent-terminal, or brew reinstall agent-terminal)",
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
    let audited = if fix { shell::repair() } else { shell::audit() };
    let audit = match audited {
        Ok(audit) => audit,
        Err(e) => return Check::problem("shell", format!("{e:#}"), "set AGENT_TERMINAL_HOME to a folder you own"),
    };
    let folder = audit.folder.display();
    let (changed, left_over) = (audit.outdated.len(), audit.leftovers.len());
    if changed + left_over > 0 && !fix {
        return Check::problem(
            "shell",
            format!(
                "the shell integration scripts in {folder} are not this version's ({changed} changed, {left_over} left over)"
            ),
            "agent-terminal doctor --fix brings them up to date",
        );
    }
    let yours = match std::env::var_os("SHELL") {
        None => Err("SHELL is not set, so `agent-terminal shell` does not know which shell to open".to_string()),
        Some(program) => {
            let program = program.to_string_lossy();
            Shell::of_program(&program).ok_or_else(|| {
                format!("SHELL is {program}, which has no shell integration (zsh, bash and fish have it)")
            })
        }
    };
    let yours = match yours {
        Ok(shell) => shell.name(),
        Err(message) => {
            return Check::problem(
                "shell",
                message,
                "open a shell with agent-terminal shell --shell zsh (or bash, fish)",
            );
        }
    };
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

fn daemon(fix: bool) -> Check {
    let socket = match connection::socket_path() {
        Ok(socket) => socket,
        Err(e) => return Check::problem("daemon", format!("{e:#}"), "set HOME, or AGENT_TERMINAL_HOME"),
    };
    match UnixStream::connect(&socket) {
        Ok(_) => Check::ok("daemon", format!("running, listening on {}", socket.display())),
        Err(e) if e.kind() == ErrorKind::NotFound => Check::ok("daemon", "not running; the next command starts it"),
        Err(e) if e.kind() == ErrorKind::ConnectionRefused && fix => match connection::remove_stale_socket() {
            Ok(true) => Check::fixed("daemon", format!("removed the stale socket {}", socket.display())),
            Ok(false) => daemon(false),
            Err(e) => Check::problem(
                "daemon",
                format!("cannot remove the stale socket {}: {e:#}", socket.display()),
                format!("remove it by hand: rm {}", socket.display()),
            ),
        },
        Err(e) if e.kind() == ErrorKind::ConnectionRefused => Check::problem(
            "daemon",
            format!("stale socket {}: no daemon is listening on it", socket.display()),
            "agent-terminal doctor --fix removes it",
        ),
        Err(e) => Check::problem(
            "daemon",
            format!("cannot reach the daemon socket {}: {e}", socket.display()),
            "set AGENT_TERMINAL_HOME to a short folder you own",
        ),
    }
}
