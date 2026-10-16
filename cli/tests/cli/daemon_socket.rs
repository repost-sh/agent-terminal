use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::process::Stdio;

use crate::support::AgentTerminal;

#[test]
fn a_second_daemon_finds_the_first_listening_and_leaves_quietly() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x2", "cat"]).success();
    let second = at.run(&["daemon", "run"]).success();
    assert_eq!((second.stdout(), second.stderr()), (String::new(), String::new()));
    assert_eq!(at.run(&["get", "program"]).success().stdout(), "cat\n", "the first daemon keeps serving");
}

#[test]
fn a_daemon_replaces_a_socket_nobody_listens_on() {
    let at = AgentTerminal::new();
    drop(UnixListener::bind(at.home().join("daemon.sock")).expect("leave a socket file behind"));
    let mut daemon = at.command(&["daemon", "run"]).stdout(Stdio::piped()).spawn().expect("start the daemon");
    let mut ready = String::new();
    BufReader::new(daemon.stdout.take().expect("the daemon's stdout"))
        .read_line(&mut ready)
        .expect("read its first line");
    assert_eq!(ready, "ready\n");
    at.run(&["open", "--size", "20x2", "cat"]).success();
    at.run(&["daemon", "stop"]).success();
    assert!(daemon.wait().expect("wait for the daemon").success());
}

#[test]
fn a_daemon_that_cannot_listen_says_where() {
    let at = AgentTerminal::new();
    let mut locked = std::fs::metadata(at.home()).expect("the home folder").permissions();
    locked.set_mode(0o500);
    std::fs::set_permissions(at.home(), locked.clone()).expect("make the home read-only");
    let refused = at.run(&["daemon", "run"]);
    locked.set_mode(0o700);
    std::fs::set_permissions(at.home(), locked).expect("make the home writable again");
    assert_eq!(refused.code(), 1);
    let socket = at.home().join("daemon.sock");
    assert!(refused.stderr().starts_with(&format!("error: listen on {}: ", socket.display())), "{}", refused.stderr());
}
