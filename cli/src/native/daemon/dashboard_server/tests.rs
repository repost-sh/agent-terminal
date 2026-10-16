use std::io::{Read, Write};
use std::net::TcpStream;

use serde_json::{Value, json};

use crate::native::daemon::tests::{ECHO, Running};

fn start(port: u64, origins: &[&str]) -> Value {
    json!({ "command": "dashboard-start", "port": port, "origins": origins })
}

fn http(port: u64, method: &str, path: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", u16::try_from(port).unwrap())).unwrap();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: localhost:{port}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    (head.split_whitespace().nth(1).unwrap().parse().unwrap(), body.to_string())
}

#[test]
fn starting_again_returns_the_running_dashboard_unless_it_asks_for_another_port_or_origin() {
    let running = Running::new();
    let started = running.ok(&start(0, &["http://a.test"]));
    let port = started["port"].as_u64().unwrap();
    assert_eq!(started["url"], format!("http://localhost:{port}"));
    assert_eq!([running.ok(&start(0, &["http://a.test"])), running.ok(&start(port, &[]))], [started.clone(), started]);
    let refusal = format!(
        "the dashboard already runs at http://localhost:{port}; stop it first with `agent-terminal dashboard stop`"
    );
    assert_eq!(
        [running.refused(&start(0, &["http://b.test"])), running.refused(&start(port + 1, &[]))],
        [(1, refusal.clone()), (1, refusal)]
    );
    let stop = json!({ "command": "dashboard-stop" });
    assert_eq!([running.ok(&stop), running.ok(&stop)], [json!({ "stopped": true }), json!({ "stopped": false })]);
}

#[test]
fn the_dashboard_refuses_a_tab_that_is_not_a_tab_id() {
    let running = Running::new();
    running.open(ECHO);
    let port = running.ok(&start(0, &[]))["port"].as_u64().unwrap();
    assert_eq!(
        [http(port, "GET", "/api/session/s/zz/commands/1"), http(port, "POST", "/api/session/s/zz/close")],
        [
            (404, "zz is not a tab id; tabs are t1, t2, ...".to_string()),
            (404, "zz is not a tab id in session s; tabs are t1, t2, ...".to_string())
        ]
    );
}
