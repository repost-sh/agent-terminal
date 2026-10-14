use std::io::{Read, Write};
use std::net::TcpStream;

use crate::support::{AgentTerminal, fixture};

struct Http {
    status: u16,
    head: String,
    body: String,
}

fn exchange(port: u16, request: &str) -> Http {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to the dashboard");
    stream.write_all(request.as_bytes()).expect("send the request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read the response");
    let (head, body) = response.split_once("\r\n\r\n").expect("an HTTP response");
    let status = head.split_whitespace().nth(1).and_then(|code| code.parse().ok()).expect("a status code");
    Http { status, head: head.to_ascii_lowercase(), body: body.to_string() }
}

fn get(port: u16, path: &str, extra: &[(&str, &str)]) -> Http {
    let mut request = format!("GET {path} HTTP/1.1\r\nHost: localhost:{port}\r\nConnection: close\r\n");
    for (name, value) in extra {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    exchange(port, &request)
}

fn start(at: &AgentTerminal) -> u16 {
    let started = at.run(&["--json", "dashboard", "start", "--port", "0"]).success().json();
    let port = started["port"].as_u64().expect("the port the dashboard listens on") as u16;
    assert_eq!(started, serde_json::json!({ "url": format!("http://localhost:{port}"), "port": port }));
    port
}

#[test]
fn dashboard_start_serves_the_web_app_built_into_the_binary() {
    let at = AgentTerminal::new();
    let port = start(&at);

    let page = get(port, "/", &[]);
    assert_eq!(page.status, 200, "{}", page.head);
    assert!(page.head.contains("content-type: text/html"), "{}", page.head);
    assert!(page.body.contains("<title>agent-terminal</title>"), "{}", page.body);

    let script = page.body.split("src=\"").nth(1).and_then(|rest| rest.split('"').next()).expect("a script tag");
    let asset = get(port, script, &[]);
    assert_eq!(asset.status, 200, "{script}: {}", asset.head);
    assert!(asset.head.contains("content-type: text/javascript"), "{}", asset.head);
}

#[test]
fn the_app_answers_every_page_path_and_only_known_api_paths() {
    let at = AgentTerminal::new();
    let port = start(&at);

    let tab_page = get(port, "/s/qa/t1", &[]);
    assert_eq!(tab_page.status, 200, "{}", tab_page.head);
    assert!(tab_page.body.contains("<title>agent-terminal</title>"), "{}", tab_page.body);

    assert_eq!(get(port, "/api/nothing-here", &[]).status, 404);
    assert_eq!(get(port, "/missing.js", &[]).status, 404);
}

#[test]
fn the_api_lists_the_same_sessions_as_session_list() {
    let at = AgentTerminal::new();
    at.run(&[
        "--session",
        "qa",
        "open",
        "--size",
        "30x4",
        "--wait",
        "line 2",
        &fixture("line-printer"),
        "--lines",
        "2",
    ])
    .success();
    let port = start(&at);

    let listed = get(port, "/api/sessions", &[]);
    assert_eq!(listed.status, 200, "{}", listed.head);
    assert!(listed.head.contains("content-type: application/json"), "{}", listed.head);
    let api: serde_json::Value = serde_json::from_str(&listed.body).expect("JSON");
    assert_eq!(api, at.run(&["--json", "session", "list"]).success().json());
    assert_eq!(api["sessions"][0]["tabs"][0]["program"], "line-printer");
}

#[test]
fn the_api_serves_a_shell_command_with_its_colors_kept() {
    let at = AgentTerminal::new();
    let home = at.home().join("user");
    std::fs::create_dir_all(&home).expect("create the home folder");
    let cwd = home.canonicalize().expect("canonical home").to_string_lossy().into_owned();
    at.run_with_env(&["shell", "--shell", "bash", "--cwd", &cwd], &[("HOME", home.as_path())]).success();
    at.run(&["exec", "printf '\\033[32mgreen\\033[0m plain\\n'"]).success();
    let port = start(&at);

    let served = get(port, "/api/session/default/t1/commands/1", &[]);
    assert_eq!(served.status, 200, "{}", served.head);
    let mut served: serde_json::Value = serde_json::from_str(&served.body).expect("JSON");
    assert_eq!(served["output"], "\u{1b}[32mgreen\u{1b}[0m plain\n");
    let mut shown = at.run(&["--json", "history", "show", "1"]).success().json();
    assert_eq!(shown["output"], "green plain\n");
    served["output"] = serde_json::Value::Null;
    shown["output"] = serde_json::Value::Null;
    assert_eq!(served, shown);

    let missing = get(port, "/api/session/default/t1/commands/9", &[]);
    assert_eq!((missing.status, missing.body.as_str()), (404, "no command 9 in default t1"));
}

#[test]
fn the_dashboard_refuses_requests_from_other_sites() {
    let at = AgentTerminal::new();
    let port = start(&at);

    assert_eq!(get(port, "/api/sessions", &[("Origin", &format!("http://localhost:{port}"))]).status, 200);
    assert_eq!(get(port, "/api/sessions", &[("Origin", &format!("http://127.0.0.1:{port}"))]).status, 200);

    let foreign = get(port, "/api/sessions", &[("Origin", "http://evil.example")]);
    assert_eq!(foreign.status, 403);
    assert_eq!(foreign.body, format!("the dashboard only answers its own pages at http://localhost:{port}"));

    let rebound = exchange(port, "GET /api/sessions HTTP/1.1\r\nHost: evil.example\r\nConnection: close\r\n\r\n");
    assert_eq!(rebound.status, 403);
    let closing = exchange(
        port,
        &format!(
            "POST /api/session/qa/t1/close HTTP/1.1\r\nHost: localhost:{port}\r\nOrigin: http://evil.example\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ),
    );
    assert_eq!(closing.status, 403);
}

#[test]
fn dashboard_stop_closes_the_port_and_says_when_none_runs() {
    let at = AgentTerminal::new();
    let started = at.run(&["dashboard", "start", "--port", "0"]).success().stdout();
    let port: u16 = started.trim().rsplit(':').next().and_then(|p| p.parse().ok()).expect("a port");
    assert_eq!(started, format!("dashboard running at http://localhost:{port}\n"));

    assert_eq!(at.run(&["dashboard", "stop"]).success().stdout(), "dashboard stopped\n");
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err(), "port {port} still answers");
    assert_eq!(at.run(&["dashboard", "stop"]).success().stdout(), "no dashboard is running\n");
    assert_eq!(at.run(&["--json", "dashboard", "stop"]).success().json(), serde_json::json!({ "stopped": false }));

    at.run(&["daemon", "stop"]).success();
    assert_eq!(at.run(&["dashboard", "stop"]).success().stdout(), "no dashboard is running\n");
}

#[test]
fn starting_again_keeps_the_running_dashboard_and_refuses_another_port() {
    let at = AgentTerminal::new();
    let port = start(&at);
    let running = serde_json::json!({ "url": format!("http://localhost:{port}"), "port": port });

    assert_eq!(at.run(&["--json", "dashboard", "start", "--port", &port.to_string()]).success().json(), running);
    assert_eq!(at.run(&["--json", "dashboard", "--port", "0"]).success().json(), running);

    let free = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("find a free port");
    let other = free.local_addr().expect("its address").port().to_string();
    drop(free);
    let refused = at.run(&["dashboard", "start", "--port", &other]);
    assert_eq!(refused.code(), 1);
    assert_eq!(
        refused.stderr(),
        format!(
            "error: the dashboard already runs at http://localhost:{port}; stop it first with `agent-terminal dashboard stop`\n"
        )
    );
}

#[test]
fn starting_on_a_port_another_program_holds_fails() {
    let at = AgentTerminal::new();
    let taken = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("hold a port");
    let port = taken.local_addr().expect("its address").port().to_string();

    let refused = at.run(&["dashboard", "start", "--port", &port]);
    assert_eq!(refused.code(), 1);
    assert_eq!(
        refused.stderr(),
        format!(
            "error: port {port} is in use by another program; pick another with `agent-terminal dashboard start --port <n>`\n"
        )
    );
}
