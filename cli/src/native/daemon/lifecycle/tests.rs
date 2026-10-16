use std::time::{Duration, Instant};

use serde_json::json;

use crate::home::{DAEMON_LOCK, DAEMON_VERSION, SOCKET};
use crate::native::daemon::tests::{ECHO, Running};

const STOPPING: &str = "the daemon is stopping; run the command again to start a new one";

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
fn a_stopped_daemon_has_let_go_of_its_lock_by_the_time_it_answers() {
    let running = Running::new();
    let lock = std::fs::File::open(running.home.path().join(DAEMON_LOCK)).unwrap();
    assert!(lock.try_lock().is_err(), "the running daemon holds its lock");
    assert_eq!(running.respond(None, &json!({ "command": "stop" })), json!({ "Ok": { "reply": "Stopped" } }));
    assert!(lock.try_lock().is_ok(), "the next daemon can take the lock");
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

fn stopped(reply: &crate::protocol::Reply) -> serde_json::Value {
    serde_json::to_value(reply).unwrap()
}

#[test]
fn a_stop_without_terminals_answers_at_once_when_no_other_call_is_in_flight() {
    let running = Running::new();
    let asking = running.daemon.busy().unwrap();
    let started = Instant::now();
    let reply = running.daemon.stop_if_no_terminals(Duration::from_secs(10));
    assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
    drop(asking);
    assert_eq!(stopped(&reply), json!({ "reply": "Stopped" }));
}

#[test]
fn a_stop_without_terminals_waits_its_time_for_a_call_in_flight_that_opens_none() {
    let running = Running::new();
    let (in_flight, asking) = (running.daemon.busy().unwrap(), running.daemon.busy().unwrap());
    let started = Instant::now();
    let reply = running.daemon.stop_if_no_terminals(Duration::from_millis(50));
    assert!(started.elapsed() >= Duration::from_millis(50), "{:?}", started.elapsed());
    drop((in_flight, asking));
    assert_eq!(stopped(&reply), json!({ "reply": "Stopped" }));
}

#[test]
fn a_stop_without_terminals_keeps_running_once_a_call_in_flight_opens_one() {
    let running = Running::new();
    let (in_flight, asking) = (running.daemon.busy().unwrap(), running.daemon.busy().unwrap());
    let reply = std::thread::scope(|scope| {
        let stopping = scope.spawn(|| running.daemon.stop_if_no_terminals(Duration::from_secs(60)));
        running.open(ECHO);
        stopping.join().unwrap()
    });
    drop((in_flight, asking));
    assert_eq!(stopped(&reply), json!({ "reply": "KeptRunning", "data": { "terminals": 1 } }));
}
