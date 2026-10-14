use std::io::ErrorKind;
use std::os::unix::net::UnixStream;

use serde::Serialize;

use crate::connection;
use crate::native::{pty, render};

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
    let checks = [pty(), fonts(), daemon(fix)];
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
            "reinstall agent-terminal (npm i -g agent-terminal, or brew reinstall agent-terminal)",
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
