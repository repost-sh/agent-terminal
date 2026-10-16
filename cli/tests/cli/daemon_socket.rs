use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::{Child, Stdio};

use crate::support::{AgentTerminal, BUILD};

fn started(at: &AgentTerminal) -> Child {
    let mut daemon = at.command(&["daemon", "run"]).stdout(Stdio::piped()).spawn().expect("start the daemon");
    let mut ready = String::new();
    BufReader::new(daemon.stdout.take().expect("the daemon's stdout"))
        .read_line(&mut ready)
        .expect("read its first line");
    assert_eq!(ready, "ready\n");
    daemon
}

#[test]
fn a_second_daemon_is_refused_naming_the_pid_build_and_socket_of_the_one_that_runs() {
    let at = AgentTerminal::without_session();
    let mut first = started(&at);
    let second = at.run(&["daemon", "run"]);
    assert_eq!((second.code(), second.stdout()), (1, String::new()));
    assert_eq!(
        second.stderr(),
        format!(
            "error: another agent-terminal daemon runs for this user: pid {}, agent-terminal {BUILD}, listening on {}\n",
            first.id(),
            at.home().join("daemon.sock").display()
        )
    );
    at.run(&["session", "new", "work"]).success();
    at.run(&["--session", "work", "open", "--size", "20x2", "cat"]).success();
    assert_eq!(
        at.run(&["--session", "work", "get", "program"]).success().stdout(),
        "cat\n",
        "the first daemon keeps serving"
    );
    at.run(&["daemon", "stop"]).success();
    assert!(first.wait().expect("wait for the daemon").success());
}

#[test]
fn a_daemon_is_refused_when_one_that_takes_no_lock_answers_on_the_socket() {
    let at = AgentTerminal::without_session();
    std::fs::write(at.home().join("daemon.version"), "0.0.1").expect("record the other daemon's build");
    let _other = UnixListener::bind(at.home().join("daemon.sock")).expect("listen where the daemon listens");
    let refused = at.run(&["daemon", "run"]);
    assert_eq!((refused.code(), refused.stdout()), (1, String::new()));
    assert_eq!(
        refused.stderr(),
        format!(
            "error: another agent-terminal daemon runs for this user: pid unknown, agent-terminal 0.0.1, listening on {}\n",
            at.home().join("daemon.sock").display()
        )
    );
}

#[test]
fn a_daemon_is_refused_while_another_holds_the_lock_before_writing_its_pid_or_build() {
    let at = AgentTerminal::without_session();
    let lock = std::fs::File::create(at.home().join("daemon.lock")).expect("create the lock");
    lock.try_lock().expect("hold the lock as a daemon that is starting does");
    let refused = at.run(&["daemon", "run"]);
    assert_eq!((refused.code(), refused.stdout()), (1, String::new()));
    assert_eq!(
        refused.stderr(),
        format!(
            "error: another agent-terminal daemon runs for this user: pid unknown, an agent-terminal that does not record its build, listening on {}\n",
            at.home().join("daemon.sock").display()
        )
    );
}

#[test]
fn a_daemon_replaces_a_socket_nobody_listens_on() {
    let at = AgentTerminal::without_session();
    drop(UnixListener::bind(at.home().join("daemon.sock")).expect("leave a socket file behind"));
    let mut daemon = started(&at);
    at.run(&["session", "new", "work"]).success();
    at.run(&["--session", "work", "open", "--size", "20x2", "cat"]).success();
    at.run(&["daemon", "stop"]).success();
    assert!(daemon.wait().expect("wait for the daemon").success());
}

fn read_only(home: &Path) -> impl FnOnce() {
    let mut locked = std::fs::metadata(home).expect("the home folder").permissions();
    locked.set_mode(0o500);
    std::fs::set_permissions(home, locked.clone()).expect("make the home read-only");
    let home = home.to_path_buf();
    move || {
        locked.set_mode(0o700);
        std::fs::set_permissions(&home, locked).expect("make the home writable again");
    }
}

#[test]
fn a_daemon_that_cannot_open_its_lock_says_where() {
    let at = AgentTerminal::without_session();
    let writable_again = read_only(at.home());
    let refused = at.run(&["daemon", "run"]);
    writable_again();
    assert_eq!(refused.code(), 1);
    let lock = at.home().join("daemon.lock");
    assert!(refused.stderr().starts_with(&format!("error: open {}: ", lock.display())), "{}", refused.stderr());
}

#[test]
fn a_daemon_that_cannot_listen_says_where() {
    let at = AgentTerminal::without_session();
    std::fs::write(at.home().join("daemon.lock"), "").expect("leave the lock of an earlier daemon behind");
    let writable_again = read_only(at.home());
    let refused = at.run(&["daemon", "run"]);
    writable_again();
    assert_eq!(refused.code(), 1);
    let socket = at.home().join("daemon.sock");
    assert!(refused.stderr().starts_with(&format!("error: listen on {}: ", socket.display())), "{}", refused.stderr());
}
