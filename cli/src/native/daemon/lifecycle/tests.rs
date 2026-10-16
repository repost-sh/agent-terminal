use std::time::Duration;

use serde_json::json;

use crate::home::{DAEMON_VERSION, SOCKET};
use crate::native::daemon::tests::{ECHO, Running};

const STOPPING: &str = "the daemon is stopping because it was idle; run the command again to start a new one";

#[test]
fn a_stop_closes_every_tab_removes_the_socket_and_version_files_and_ends_the_daemon() {
    let running = Running::new();
    running.open(ECHO);
    for file in [SOCKET, DAEMON_VERSION] {
        std::fs::write(running.home.path().join(file), "").unwrap();
    }
    assert_eq!(running.respond(None, &json!({ "command": "stop" })), json!({ "Ok": { "reply": "Stopped" } }));
    assert_eq!(running.ended.try_recv(), Ok(()));
    let left = [SOCKET, DAEMON_VERSION].map(|file| running.home.path().join(file).exists());
    assert_eq!((left, running.ok(&json!({ "command": "sessions" }))), ([false, false], json!({ "sessions": [] })));
}

#[test]
fn a_daemon_with_terminals_keeps_running_when_asked_to_stop_only_without_them() {
    let running = Running::new();
    running.open(ECHO);
    let stop_if_empty = json!({ "command": "stop-if-no-terminals" });
    assert_eq!(running.ok(&stop_if_empty), json!({ "terminals": 1 }));
    running.daemon.close_all();
    assert_eq!(running.respond(None, &stop_if_empty), json!({ "Ok": { "reply": "Stopped" } }));
    assert_eq!(running.refused(&json!({ "command": "sessions" })), (1, STOPPING.to_string()));
}

#[test]
fn an_idle_daemon_stops_and_refuses_every_call_after() {
    let running = Running::new();
    running.open(ECHO);
    running.daemon.stop_when_idle(Duration::ZERO);
    assert_eq!(running.ended.try_recv(), Ok(()));
    assert_eq!(running.refused(&json!({ "command": "sessions" })), (1, STOPPING.to_string()));
    assert!(running.daemon.busy().is_none());
}
