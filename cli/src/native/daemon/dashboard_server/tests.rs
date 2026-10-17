use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::{Value, json};

use crate::native::daemon::tests::{ECHO, Running, open, rename};

fn start(port: u64, origins: &[&str]) -> Value {
    json!({ "command": "dashboard-start", "port": port, "origins": origins })
}

fn send(port: u64, request: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", u16::try_from(port).unwrap())).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    (head.split_whitespace().nth(1).unwrap().parse().unwrap(), body.to_string())
}

fn http(port: u64, method: &str, path: &str) -> (u16, String) {
    send(
        port,
        &format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost:{port}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ),
    )
}

fn json_of(answer: &(u16, String)) -> (u16, Value) {
    (answer.0, serde_json::from_str(&answer.1).unwrap())
}

fn opened(port: u64, opening: &Value) -> (u16, Value) {
    let body = opening.to_string();
    json_of(&send(
        port,
        &format!(
            "POST /api/sessions HTTP/1.1\r\nHost: localhost:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
    ))
}

fn opening(session: &str, command: &str) -> Value {
    json!({ "session": session, "new_session": true, "tab": "app", "command": command, "cwd": "/", "cols": 30, "rows": 4, "shell_integration": false })
}

#[test]
fn starting_again_returns_the_running_dashboard_whatever_port_or_origin_it_asks_for() {
    let running = Running::new();
    let started = running.ok(&start(0, &["http://a.test"]));
    let port = started["port"].as_u64().unwrap();
    assert_eq!(started["url"], format!("http://localhost:{port}"));
    assert_eq!(
        [start(0, &["http://a.test"]), start(port, &[]), start(0, &["http://b.test"]), start(port + 1, &[])]
            .map(|again| running.ok(&again)),
        [started.clone(), started.clone(), started.clone(), started]
    );
    assert_eq!(
        send(
            port,
            "GET /api/sessions HTTP/1.1\r\nHost: localhost\r\nOrigin: http://b.test\r\nConnection: close\r\n\r\n"
        )
        .0,
        403
    );
    let stop = json!({ "command": "dashboard-stop" });
    assert_eq!([running.ok(&stop), running.ok(&stop)], [json!({ "stopped": true }), json!({ "stopped": false })]);
}

#[test]
fn the_dashboard_refuses_a_tab_that_is_not_a_tab_name() {
    let running = Running::new();
    running.open("echo", ECHO);
    let port = running.ok(&start(0, &[]))["port"].as_u64().unwrap();
    assert_eq!(
        [http(port, "GET", "/api/session/s/t1/commands/1"), http(port, "POST", "/api/session/s/Echo/close")],
        [
            (404, "s t1 is not open".to_string()),
            (404, format!("\"Echo\" is not a name: a name is {}, such as repost-forward", crate::protocol::NAME_RULE))
        ]
    );
}

#[test]
fn the_dashboard_opens_and_lists_sessions_and_says_why_it_cannot_open_one() {
    let running = Running::new();
    let port = running.ok(&start(0, &[]))["port"].as_u64().unwrap();
    let (status, reply) = opened(port, &opening("n", "/bin/sh -c 'printf ready; read x'"));
    assert_eq!((status, &reply["reply"], &reply["data"]["tab"]), (200, &json!("Snapshot"), &json!("app")));
    let listed = json_of(&http(port, "GET", "/api/sessions")).1;
    assert_eq!(
        (&listed["sessions"][0]["tabs"][0]["program"], &listed["sessions"][1]["tabs"]),
        (&json!("sh"), &json!([]))
    );
}

#[test]
fn the_dashboard_opens_a_tab_in_a_session_it_has_and_refuses_a_taken_name_or_a_program_that_cannot_start() {
    let running = Running::new();
    let port = running.ok(&start(0, &[]))["port"].as_u64().unwrap();
    let mut in_s = opening("s", "/bin/sh -c 'printf again; read x'");
    in_s["new_session"] = json!(false);
    assert_eq!(opened(port, &in_s).1["data"]["tab"], "app");
    assert_eq!(
        opened(port, &in_s),
        (422, json!({ "error": { "code": 1, "message": "tab app already runs sh in session s; pick another name" } }))
    );
    running.open("echo", ECHO);
    let (taken, refusal) = opened(port, &opening("s", "/bin/sh"));
    let message = refusal["error"]["message"].as_str().unwrap();
    assert_eq!(
        (taken, message.split_once(", ").map(|(_, tabs)| tabs)),
        (422, Some("2 tabs: app, echo; pick another name for this conversation"))
    );
    assert_eq!(
        opened(port, &opening("t", "/nope")),
        (422, json!({ "error": { "code": 1, "message": "command not found: /nope" } }))
    );
    let names = json_of(&http(port, "GET", "/api/sessions")).1["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|session| session["session"].clone())
        .collect::<Vec<_>>();
    assert_eq!(names, [json!("s")], "a session made for a program that cannot start is ended again");
}

#[test]
fn the_dashboard_closes_a_tab_or_every_session_and_restarts_a_tab() {
    let running = Running::new();
    running.open("app", ECHO);
    running.open("api", ECHO);
    let port = running.ok(&start(0, &[]))["port"].as_u64().unwrap();
    let (restarted, reply) = json_of(&http(port, "POST", "/api/session/s/app/restart"));
    assert_eq!((restarted, &reply["reply"], &reply["data"]["lines"][0]), (200, &json!("Snapshot"), &json!("ready")));
    let missing = json_of(&http(port, "POST", "/api/session/s/db/close"));
    assert_eq!(
        (missing.0, &missing.1["error"]["message"]),
        (422, &json!("no tab db in session s; list tabs with `agent-terminal tab`"))
    );
    assert_eq!(
        [json_of(&http(port, "POST", "/api/session/s/app/close")), json_of(&http(port, "POST", "/api/sessions/close"))],
        [
            (200, json!({ "reply": "Closed", "data": { "closed": [{ "session": "s", "tab": "app" }] } })),
            (
                200,
                json!({ "reply": "Closed", "data": { "closed": [{ "session": "s", "tab": "api" }], "ended": ["s"] } })
            )
        ]
    );
}

#[test]
fn the_dashboard_answers_only_its_own_pages_and_the_origins_it_was_given() {
    let running = Running::new();
    let port = running.ok(&start(0, &["https://ok.test"]))["port"].as_u64().unwrap();
    let from = |host: &str, origin: &str| {
        send(port, &format!("GET /api/sessions HTTP/1.1\r\nHost: {host}\r\n{origin}Connection: close\r\n\r\n")).0
    };
    let ours = format!("localhost:{port}");
    assert_eq!(
        [
            from(&ours, ""),
            from(&format!("127.0.0.1:{port}"), &format!("Origin: http://localhost:{port}\r\n")),
            from(&ours, "Origin: https://ok.test\r\n"),
            from(&ours, "Origin: https://evil.test\r\n"),
            from(&format!("evil.test:{port}"), ""),
            from(&format!("localhost:{}", port + 1), "")
        ],
        [200, 200, 200, 403, 403, 403]
    );
    let refused = send(port, &format!("GET / HTTP/1.1\r\nHost: evil.test:{port}\r\nConnection: close\r\n\r\n"));
    assert_eq!(refused, (403, format!("the dashboard only answers its own pages at http://localhost:{port}")));
}

fn feed(port: u64, path: &str) -> tungstenite::WebSocket<TcpStream> {
    let stream = TcpStream::connect(("127.0.0.1", u16::try_from(port).unwrap())).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    tungstenite::client(format!("ws://localhost:{port}{path}").as_str(), stream).unwrap().0
}

fn next(socket: &mut tungstenite::WebSocket<TcpStream>) -> Value {
    match socket.read().unwrap() {
        tungstenite::Message::Text(text) => serde_json::from_str(&text).unwrap(),
        other => panic!("expected a text message, got {other:?}"),
    }
}

#[test]
fn the_info_feed_describes_the_tab_and_says_closed_when_it_closes() {
    let running = Running::new();
    let mut secret = open("echo", ECHO);
    secret["env"] = json!([["PATH", "/usr/bin:/bin"], ["API_TOKEN", "hidden"]]);
    running.ok(&secret);
    let port = running.ok(&start(0, &[]))["port"].as_u64().unwrap();
    let mut info = feed(port, "/api/session/s/echo/info");
    let mut described = next(&mut info);
    assert!(described["pid"].take().as_u64().is_some_and(|pid| pid > 0) && described["started"].take().is_string());
    insta::assert_snapshot!(described, @r#"{"args":"-c \"printf 'ready\\\\n'; while read line; do printf 'got %s\\\\n' \\\"\"'$line\"; done'","cols":30,"cwd":"/","env":[{"name":"API_TOKEN","value":null},{"name":"COLORTERM","value":"truecolor"},{"name":"PATH","value":"/usr/bin:/bin"},{"name":"TERM","value":"xterm-256color"}],"exit_code":null,"exited":false,"modes":{"alt_screen":false,"app_cursor":false,"bracketed_paste":false,"focus_events":false,"keyboard":[],"mouse":null},"pid":null,"program":"/bin/sh","rows":4,"shell_integration":null,"started":null,"term":"xterm-256color","type":"info"}"#);
    running.ok(&on_close());
    assert_eq!(next(&mut info), json!({ "type": "closed" }));
}

fn on_close() -> Value {
    json!({ "command": "close", "session": "s", "tab": null, "scope": "tab" })
}

#[test]
fn a_renamed_tab_keeps_its_activity_and_a_new_tab_with_its_old_name_starts_without() {
    let running = Running::new();
    running.respond(Some(&["open", "--name", "echo", "sh"]), &open("echo", ECHO));
    running.respond(Some(&["tab", "rename", "web"]), &rename(None, "web"));
    running.respond(Some(&["open", "--name", "echo", "sh"]), &open("echo", ECHO));
    let port = running.ok(&start(0, &[]))["port"].as_u64().unwrap();
    let listed = |tab: &str| {
        let update = next(&mut feed(port, &format!("/api/session/s/{tab}/activity")));
        let entries = update["entries"].as_array().unwrap().iter();
        entries
            .map(|entry| format!("{} | {}", entry["tab"].as_str().unwrap(), entry["command"].as_str().unwrap()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        [listed("web"), listed("echo")],
        [
            vec!["echo | open --name echo sh".to_string(), "web | tab rename web".to_string()],
            vec!["echo | open --name echo sh".to_string()]
        ]
    );
}
