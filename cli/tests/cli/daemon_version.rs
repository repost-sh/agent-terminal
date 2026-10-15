use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::thread::JoinHandle;

use serde_json::{Value, json};

use crate::support::AgentTerminal;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const STOPPED: &str = r#"{"Ok":{"reply":"Stopped"}}"#;

fn write_message(stream: &mut UnixStream, body: &[u8]) {
    let length = u32::try_from(body.len()).expect("a message shorter than 4 GiB").to_be_bytes();
    stream.write_all(&length).and_then(|()| stream.write_all(body)).expect("write a message");
}

fn read_message(stream: &mut UnixStream) -> Option<Value> {
    let mut length = [0u8; 4];
    stream.read_exact(&mut length).ok()?;
    let mut body = vec![0u8; u32::from_be_bytes(length) as usize];
    stream.read_exact(&mut body).expect("read the message");
    Some(serde_json::from_slice(&body).expect("the message is JSON"))
}

fn stand_in(home: &Path, version: Option<&str>, reply: Option<&'static str>) -> JoinHandle<Vec<Value>> {
    let socket = home.join("daemon.sock");
    let version_file = home.join("daemon.version");
    if let Some(version) = version {
        std::fs::write(&version_file, version).expect("record the stand-in's version");
    }
    let listener = UnixListener::bind(&socket).expect("listen where the daemon listens");
    std::thread::spawn(move || {
        let mut received = Vec::new();
        for stream in listener.incoming() {
            let mut stream = stream.expect("accept a connection");
            let Some(request) = read_message(&mut stream) else {
                continue;
            };
            received.push(request);
            if let Some(reply) = reply {
                std::fs::remove_file(&version_file).expect("remove the version file as a stopping daemon does");
                std::fs::remove_file(&socket).expect("remove the socket as a stopping daemon does");
                write_message(&mut stream, reply.as_bytes());
            }
            break;
        }
        received
    })
}

fn stop_if_no_terminals() -> Value {
    json!({ "typed": null, "request": { "command": "stop-if-no-terminals" } })
}

fn recorded_version(at: &AgentTerminal) -> String {
    std::fs::read_to_string(at.home().join("daemon.version")).expect("the daemon records its version")
}

#[test]
fn a_command_replaces_a_daemon_of_another_version_that_has_no_terminals() {
    let at = AgentTerminal::new();
    let other = stand_in(at.home(), Some("0.0.1"), Some(STOPPED));

    let run = at.run(&["open", "--size", "20x2", "cat"]).success();
    assert_eq!(run.stderr(), "");
    assert_eq!(other.join().unwrap(), [stop_if_no_terminals()], "the other version gets nothing but the stop");
    assert_eq!(recorded_version(&at), VERSION);
    assert_eq!(at.run(&["get", "program"]).success().stdout(), "cat\n");
}

#[test]
fn commands_started_together_replace_a_daemon_of_another_version_once() {
    let at = AgentTerminal::new();
    let other = stand_in(at.home(), Some("0.0.1"), Some(STOPPED));
    std::thread::scope(|scope| {
        let opens: Vec<_> = (0..3).map(|_| scope.spawn(|| at.run(&["open", "--size", "20x2", "cat"]).code())).collect();
        let codes: Vec<i32> = opens.into_iter().map(|open| open.join().unwrap()).collect();
        assert_eq!(codes, vec![0; 3]);
    });
    assert_eq!(other.join().unwrap(), [stop_if_no_terminals()]);
    let closed = at.run(&["close", "--all"]).success().stdout();
    assert_eq!(closed.lines().count(), 3, "every open must land in the same daemon:\n{closed}");
}

#[test]
fn a_daemon_of_another_version_with_open_terminals_is_left_running_and_the_command_exits_6() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x2", "cat"]).success();
    at.run(&["tab", "new", "--size", "20x2", "cat"]).success();
    std::fs::write(at.home().join("daemon.version"), "0.0.1").expect("make the daemon report another version");
    let message = format!(
        "the daemon is agent-terminal 0.0.1, not {VERSION} like this command, and it has 2 open terminals, so it was left running; `agent-terminal daemon stop` ends it and its terminals, and the next command starts a {VERSION} daemon"
    );

    let run = at.run(&["snapshot"]);
    assert_eq!((run.code(), run.stdout().as_str()), (6, ""));
    assert_eq!(run.stderr(), format!("error: {message}\n"));

    let run = at.run(&["--json", "snapshot"]);
    assert_eq!(run.code(), 6);
    assert_eq!(run.json(), json!({ "error": { "code": 6, "message": message } }));

    at.run(&["daemon", "stop"]).success();
    at.run(&["open", "--size", "20x2", "cat"]).success();
    assert_eq!(recorded_version(&at), VERSION);
    assert_eq!(at.run(&["close", "--all"]).success().stdout(), "closed default t1\n");
}

#[test]
fn a_daemon_that_records_no_version_and_cannot_stop_on_request_is_left_running() {
    let at = AgentTerminal::new();
    let other = stand_in(at.home(), None, None);

    let run = at.run(&["snapshot"]);
    assert_eq!((run.code(), run.stdout().as_str()), (6, ""));
    assert_eq!(
        run.stderr(),
        format!(
            "error: the daemon is an agent-terminal that does not record its version, not {VERSION} like this command, and it did not say how many terminals it has open, so it was left running; `agent-terminal daemon stop` ends it and its terminals, and the next command starts a {VERSION} daemon\n"
        )
    );
    assert_eq!(other.join().unwrap(), [stop_if_no_terminals()]);
}

#[test]
fn a_request_the_daemon_cannot_parse_gets_an_error_naming_the_daemons_version() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x2", "cat"]).success();
    let mut daemon = UnixStream::connect(at.home().join("daemon.sock")).expect("connect to the daemon");
    write_message(&mut daemon, br#"{"typed":["future-command"],"request":{"command":"future-command"}}"#);

    assert_eq!(
        read_message(&mut daemon),
        Some(json!({
            "Err": {
                "code": 6,
                "message": format!(
                    "the daemon is agent-terminal {VERSION} and does not understand this request, so the command is another version; it has 1 open terminal: `agent-terminal daemon stop` ends it and its terminals, and the next command starts a daemon of the command's version"
                )
            }
        }))
    );
    assert_eq!(at.run(&["get", "program"]).success().stdout(), "cat\n", "the daemon keeps serving");
}
