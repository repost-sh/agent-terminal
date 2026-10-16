use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::thread::JoinHandle;

use serde_json::{Value, json};

use crate::support::{AgentTerminal, BUILD};

const STOPPED: &str = r#"{"Ok":{"reply":"Stopped"}}"#;

const OTHER_BUILD: &str = concat!(env!("CARGO_PKG_VERSION"), "+000000000000");

const RESTARTED: &str = "a human restarts the daemon when it suits them, because it holds other agents' terminals";

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

fn stand_in(home: &Path, build: Option<&str>, reply: Option<&'static str>) -> JoinHandle<Vec<Value>> {
    let socket = home.join("daemon.sock");
    let version_file = home.join("daemon.version");
    if let Some(build) = build {
        std::fs::write(&version_file, build).expect("record the stand-in's build");
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

fn recorded_build(at: &AgentTerminal) -> String {
    std::fs::read_to_string(at.home().join("daemon.version")).expect("the daemon records its build")
}

#[test]
fn a_command_replaces_a_daemon_of_another_build_that_has_no_terminals() {
    let at = AgentTerminal::without_session();
    let other = stand_in(at.home(), Some(OTHER_BUILD), Some(STOPPED));

    let run = at.run(&["session", "new", "work"]).success();
    assert_eq!(run.stderr(), "");
    assert_eq!(other.join().unwrap(), [stop_if_no_terminals()], "the other build gets nothing but the stop");
    assert_eq!(recorded_build(&at), BUILD);
    at.run(&["--session", "work", "open", "--size", "20x2", "cat"]).success();
    assert_eq!(at.run(&["--session", "work", "get", "program"]).success().stdout(), "cat\n");
}

#[test]
fn a_running_daemon_of_another_build_with_no_tab_is_replaced_by_the_next_command() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x2", "cat"]).success();
    at.run(&["close"]).success();
    std::fs::write(at.home().join("daemon.version"), OTHER_BUILD).expect("make the daemon report another build");

    let run = at.run(&["session", "new", "work"]).success();
    assert_eq!(run.stderr(), "");
    assert_eq!(recorded_build(&at), BUILD);
    assert!(at.daemon_log().contains("a command of another build stopped this daemon, which had no terminals"));
    at.run(&["open", "--size", "20x2", "cat"]).success();
    assert_eq!(
        at.run(&["--session", "work", "close", "--all"]).success().stdout(),
        "closed work t1\nended session work\n"
    );
}

#[test]
fn commands_started_together_replace_a_daemon_of_another_build_once() {
    let at = &AgentTerminal::without_session();
    let other = stand_in(at.home(), Some(OTHER_BUILD), Some(STOPPED));
    std::thread::scope(|scope| {
        let news: Vec<_> =
            ["a", "b", "c"].map(|name| scope.spawn(move || at.run(&["session", "new", name]).code())).into();
        let codes: Vec<i32> = news.into_iter().map(|new| new.join().unwrap()).collect();
        assert_eq!(codes, vec![0; 3]);
    });
    assert_eq!(other.join().unwrap(), [stop_if_no_terminals()]);
    let listed = at.run(&["session", "list"]).success().stdout();
    assert_eq!(listed, "a · no tabs\nb · no tabs\nc · no tabs\n", "every command must land in the same daemon");
}

#[test]
fn a_daemon_of_another_build_with_open_tabs_is_left_running_and_the_command_exits_6_naming_both_builds() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x2", "cat"]).success();
    at.run(&["tab", "new", "--size", "20x2", "cat"]).success();
    std::fs::write(at.home().join("daemon.version"), OTHER_BUILD).expect("make the daemon report another build");
    let message = format!(
        "the daemon is agent-terminal {OTHER_BUILD}, not {BUILD} like this command, and it has 2 open terminals, so it was left running; {RESTARTED}"
    );

    the_command_exits_6_naming_both_builds(&at, &message);
    at.run(&["daemon", "stop"]).success();
    at.run(&["session", "new", "work"]).success();
    at.run(&["open", "--size", "20x2", "cat"]).success();
    assert_eq!(recorded_build(&at), BUILD);
    assert_eq!(
        at.run(&["--session", "work", "close", "--all"]).success().stdout(),
        "closed work t1\nended session work\n"
    );
}

fn the_command_exits_6_naming_both_builds(at: &AgentTerminal, message: &str) {
    let run = at.run(&["snapshot"]);
    assert_eq!((run.code(), run.stdout().as_str()), (6, ""));
    assert_eq!(run.stderr(), format!("error: {message}\n"));
    assert!(!run.stderr().contains("daemon stop"), "an agent is never told to stop the daemon: {}", run.stderr());

    let run = at.run(&["--json", "snapshot"]);
    assert_eq!(run.code(), 6);
    assert_eq!(
        run.json(),
        json!({
            "error": {
                "code": 6,
                "message": message,
                "other_version": { "cli_version": BUILD, "daemon_version": OTHER_BUILD, "terminals": 2 }
            }
        })
    );
}

#[test]
fn a_daemon_that_records_no_build_and_cannot_stop_on_request_is_left_running() {
    let at = AgentTerminal::without_session();
    let other = stand_in(at.home(), None, None);
    let message = format!(
        "the daemon is an agent-terminal that does not record its build, not {BUILD} like this command, and it did not say how many terminals it has open, so it was left running; {RESTARTED}"
    );

    let run = at.run(&["--session", "work", "snapshot"]);
    assert_eq!((run.code(), run.stdout().as_str()), (6, ""));
    assert_eq!(run.stderr(), format!("error: {message}\n"));
    assert_eq!(other.join().unwrap(), [stop_if_no_terminals()]);
    the_json_error_says_neither_build_nor_terminals_are_known(&at, &message);
}

fn the_json_error_says_neither_build_nor_terminals_are_known(at: &AgentTerminal, message: &str) {
    std::fs::remove_file(at.home().join("daemon.sock")).expect("let the next stand-in listen");
    let other = stand_in(at.home(), None, None);
    let run = at.run(&["--session", "work", "--json", "snapshot"]);
    assert_eq!(run.code(), 6);
    assert_eq!(
        run.json(),
        json!({
            "error": {
                "code": 6,
                "message": message,
                "other_version": { "cli_version": BUILD, "daemon_version": null, "terminals": null }
            }
        })
    );
    assert_eq!(other.join().unwrap(), [stop_if_no_terminals()]);
}

#[test]
fn a_request_the_daemon_cannot_parse_gets_an_error_naming_the_daemons_build() {
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
                    "the daemon is agent-terminal {BUILD} and does not understand this request, so the command is another build; it has 1 open terminal: {RESTARTED}"
                ),
                "other_version": { "cli_version": null, "daemon_version": BUILD, "terminals": 1 }
            }
        }))
    );
    assert_eq!(at.run(&["get", "program"]).success().stdout(), "cat\n", "the daemon keeps serving");
}

#[test]
fn a_request_cut_short_is_dropped_and_the_daemon_keeps_serving() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x2", "cat"]).success();
    let mut daemon = UnixStream::connect(at.home().join("daemon.sock")).expect("connect to the daemon");
    daemon.write_all(&100u32.to_be_bytes()).and_then(|()| daemon.write_all(b"{\"ty")).expect("write part of a request");
    daemon.shutdown(std::net::Shutdown::Write).expect("end the request early");

    assert_eq!(read_message(&mut daemon), None, "a cut request gets no reply");
    assert_eq!(at.run(&["get", "program"]).success().stdout(), "cat\n", "the daemon keeps serving");
    assert!(at.daemon_log().contains("reading a request failed: "), "{}", at.daemon_log());
}

#[test]
fn a_daemon_with_no_terminals_stops_when_another_build_asks() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x2", "cat"]).success();
    at.run(&["close"]).success();
    let mut daemon = UnixStream::connect(at.home().join("daemon.sock")).expect("connect to the daemon");
    write_message(&mut daemon, stop_if_no_terminals().to_string().as_bytes());

    assert_eq!(read_message(&mut daemon), Some(serde_json::from_str::<Value>(STOPPED).unwrap()));
    assert_eq!(read_message(&mut daemon), None);
    assert!(at.daemon_log().contains("a command of another build stopped this daemon, which had no terminals"));
}
