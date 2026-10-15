use std::io::{Read, Write};
use std::net::TcpStream;

use crate::support::{AgentTerminal, Cue, fixture};

struct Http {
    status: u16,
    head: String,
    body: String,
    bytes: Vec<u8>,
}

fn exchange(port: u16, request: &str) -> Http {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to the dashboard");
    stream.write_all(request.as_bytes()).expect("send the request");
    let mut response = Vec::new();
    stream.read_to_end(&mut response).expect("read the response");
    let split = response.windows(4).position(|window| window == b"\r\n\r\n").expect("an HTTP response");
    let head = String::from_utf8_lossy(&response[..split]).to_ascii_lowercase();
    let bytes = response[split + 4..].to_vec();
    let status = head.split_whitespace().nth(1).and_then(|code| code.parse().ok()).expect("a status code");
    Http { status, head, body: String::from_utf8_lossy(&bytes).into_owned(), bytes }
}

fn post(port: u16, path: &str, body: &str) -> Http {
    let length = body.len();
    exchange(
        port,
        &format!(
            "POST {path} HTTP/1.1\r\nHost: localhost:{port}\r\nContent-Type: application/json\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n{body}"
        ),
    )
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
fn the_dashboard_serves_the_screenshot_fonts() {
    let at = AgentTerminal::new();
    let port = start(&at);
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/SymbolsNerdFontMono-Regular.ttf");
    let size = std::fs::metadata(&file).expect("the bundled Symbols Nerd Font").len() as usize;

    let font = get(port, "/fonts/SymbolsNerdFontMono-Regular.ttf", &[]);
    assert_eq!(font.status, 200, "{}", font.head);
    assert!(font.head.contains("content-type: font/ttf"), "{}", font.head);
    assert_eq!(font.bytes.len(), size);

    assert_eq!(get(port, "/fonts/missing.ttf", &[]).status, 404);
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
fn the_dashboard_has_no_record_start_or_stop() {
    let at = AgentTerminal::new();
    at.run(&["--session", "qa", "open", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    let port = start(&at);

    for action in ["start", "stop"] {
        let answer = post(port, &format!("/api/session/qa/t1/record/{action}"), "");
        assert_eq!((answer.status, answer.body.as_str()), (404, "no such dashboard API"), "record/{action}");
    }
    assert!(!at.home().join("recordings").exists());
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
fn an_origin_allowed_at_start_may_call_the_dashboard_as_its_own_pages_do() {
    let at = AgentTerminal::new();
    let started = at
        .run(&["--json", "dashboard", "start", "--port", "0", "--allow-origin", "http://localhost:5173"])
        .success()
        .json();
    let port = started["port"].as_u64().expect("the port") as u16;

    assert_eq!(get(port, "/api/sessions", &[("Origin", "http://localhost:5173")]).status, 200);
    assert_eq!(get(port, "/api/sessions", &[("Origin", &format!("http://localhost:{port}"))]).status, 200);
    assert_eq!(get(port, "/api/sessions", &[("Origin", "http://localhost:5174")]).status, 403);
    assert_eq!(get(port, "/api/sessions", &[("Origin", "http://evil.example")]).status, 403);

    let refused = at.run(&["dashboard", "start", "--port", "0", "--allow-origin", "localhost:5173"]);
    assert_eq!(refused.code(), 1, "{}", refused.stderr());
    assert!(
        refused.stderr().contains("an origin is http://host:port, like http://localhost:5173"),
        "{}",
        refused.stderr()
    );
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

fn path_segment(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (byte as char).to_string(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[test]
fn the_dashboard_serves_no_file_the_session_does_not_list_as_a_screenshot() {
    let at = AgentTerminal::new();
    at.run(&["open", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    let png = at.home().join("evidence.png");
    std::fs::write(&png, b"\x89PNG\r\n\x1a\n").expect("write a PNG file");
    let log = at.home().join("sessions/default/activity.jsonl");
    let port = start(&at);

    for (session, file) in [("default", &png), ("default", &log), ("other", &png)] {
        let path = file.to_string_lossy();
        let refused = get(port, &format!("/api/session/{session}/screenshots/{}", path_segment(&path)), &[]);
        assert_eq!(refused.status, 404, "{}", refused.head);
        assert_eq!(refused.body, format!("{path} is not a PNG screenshot in session {session}"));
    }
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

fn screenshot_url(session: &str, path: &str) -> String {
    format!("/api/session/{session}/screenshots/{}", path_segment(path))
}

fn screenshots_listed(at: &AgentTerminal, session: &str) -> Vec<serde_json::Value> {
    let path = at.home().join("sessions").join(session).join("screenshots.jsonl");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

#[test]
fn a_screenshot_taken_from_the_dashboard_is_saved_listed_without_a_call_and_served() {
    let at = AgentTerminal::new();
    at.run(&["--session", "qa", "open", "--size", "20x4", "--wait", "line 3", &fixture("line-printer"), "--hold"])
        .success();
    let frame: u64 = at.run(&["--session", "qa", "get", "frame"]).success().stdout().trim().parse().unwrap();
    let port = start(&at);

    let taken = post(port, "/api/session/qa/t1/screenshot", "");

    assert_eq!(taken.status, 200, "{}{}", taken.head, taken.body);
    let path = at.home().join("screenshots").join(format!("qa-t1-{frame}.png")).to_string_lossy().into_owned();
    let reply: serde_json::Value = serde_json::from_str(&taken.body).expect("JSON");
    assert_eq!(reply["data"]["path"], path.as_str());
    let listed = screenshots_listed(&at, "qa");
    assert_eq!(listed.len(), 1);
    assert_eq!(
        (&listed[0]["path"], &listed[0]["by"], &listed[0]["frame"]),
        (&path.clone().into(), &serde_json::Value::Null, &frame.into())
    );
    let served = get(port, &screenshot_url("qa", &path), &[]);
    assert_eq!(served.status, 200, "{}{}", served.head, served.body);
    assert_eq!(served.bytes, std::fs::read(&path).expect("read the screenshot"));
    let activity = std::fs::read_to_string(at.home().join("sessions/qa/activity.jsonl")).expect("the activity log");
    assert!(!activity.contains(&path), "{activity}");
}

#[test]
fn a_dashboard_screenshot_of_an_earlier_frame_draws_the_screen_of_that_frame() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    cue.open_line_printer(&at, &["--size", "20x4"]);
    let earlier: u64 = at.run(&["get", "frame"]).success().stdout().trim().parse().unwrap();
    let work = at.work_folder();
    at.run_in(&work, &["screenshot", "earlier.png"]).success();
    cue.print("later\n");
    let port = start(&at);

    let taken = post(port, "/api/session/default/t1/screenshot", &format!("{{\"frame\":{earlier}}}"));

    assert_eq!(taken.status, 200, "{}{}", taken.head, taken.body);
    let reply: serde_json::Value = serde_json::from_str(&taken.body).expect("JSON");
    let path = at.home().join("screenshots").join(format!("default-t1-{earlier}.png"));
    assert_eq!((&reply["data"]["path"], &reply["data"]["frame"]), (&path.to_string_lossy().into(), &earlier.into()));
    assert!(
        std::fs::read(&path).expect("read the screenshot") == std::fs::read(work.join("earlier.png")).unwrap(),
        "the screenshot of frame {earlier} is not the screen the tab showed then"
    );
    let listed = screenshots_listed(&at, "default");
    assert_eq!(listed.last().map(|shot| &shot["frame"]), Some(&earlier.into()));

    let unknown = post(port, "/api/session/default/t1/screenshot", "{\"frame\":99}");
    assert_eq!(unknown.status, 422, "{}{}", unknown.head, unknown.body);
    assert!(unknown.body.contains("frame 99 is not on the timeline of default t1"), "{}", unknown.body);
}

#[test]
fn the_dashboard_counts_and_boxes_the_cells_two_screenshots_differ_in() {
    let at = AgentTerminal::new();
    let shots = at.home().join("shots");
    let [before, after, bigger] =
        ["before.png", "after.png", "bigger.png"].map(|name| shots.join(name).to_string_lossy().into_owned());
    let lines = "stty -echo; printf '\\033[?25lone'; read _; printf '\\ntwo'; read _";
    at.run(&["open", "--size", "20x4", "--wait", "one", "sh", "-c", lines]).success();
    at.run(&["screenshot", &before]).success();
    at.run(&["press", "Enter", "--wait", "two", "--screenshot", &after]).success();
    at.run(&["screenshot", "--scale", "2", &bigger]).success();
    let port = start(&at);
    let compare = |a: &str, b: &str| {
        let pair = serde_json::json!({ "paths": [a, b] }).to_string();
        let compared = post(port, "/api/session/default/screenshots/compare", &pair);
        match compared.status {
            200 => serde_json::from_str(&compared.body).expect("JSON"),
            status => serde_json::json!({ "status": status, "error": compared.body }),
        }
    };

    let three = serde_json::json!({ "changed": 3, "boxes": [{ "x": 10, "y": 30, "width": 27, "height": 20 }] });
    assert_eq!(compare(&before, &after), three);
    assert_eq!(compare(&after, &before), three);
    assert_eq!(compare(&before, &before), serde_json::json!({ "changed": 0, "boxes": [] }));
    assert_eq!(compare(&before, &bigger), serde_json::json!({ "changed": null, "boxes": [] }));
    let elsewhere = shots.join("elsewhere.png").to_string_lossy().into_owned();
    let refusal = format!("{elsewhere} is not a PNG screenshot in session default");
    assert_eq!(compare(&before, &elsewhere), serde_json::json!({ "status": 404, "error": refusal }));

    at.run(&["screenshot", "--scale", "2", &before]).success();
    assert_eq!(compare(&before, &after), serde_json::json!({ "changed": null, "boxes": [] }));
}
