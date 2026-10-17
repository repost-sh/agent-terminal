use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::TcpStream;

use crate::support::{AgentTerminal, fixture};

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
        write!(request, "{name}: {value}\r\n").expect("write a header");
    }
    request.push_str("\r\n");
    exchange(port, &request)
}

fn start(at: &AgentTerminal) -> u16 {
    let started = at.run(&["--json", "dashboard", "start", "--port", "0"]).success().json();
    let port = u16::try_from(started["port"].as_u64().expect("the port the dashboard listens on")).expect("a port");
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

    let tab_page = get(port, "/s/qa/app", &[]);
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
    let size = usize::try_from(std::fs::metadata(&file).expect("the bundled Symbols Nerd Font").len()).expect("a size");

    let font = get(port, "/fonts/SymbolsNerdFontMono-Regular.ttf", &[]);
    assert_eq!(font.status, 200, "{}", font.head);
    assert!(font.head.contains("content-type: font/ttf"), "{}", font.head);
    assert_eq!(font.bytes.len(), size);

    assert_eq!(get(port, "/fonts/missing.ttf", &[]).status, 404);
}

#[test]
fn the_api_lists_the_same_sessions_as_session_list() {
    let at = AgentTerminal::new();
    at.run(&["session", "new", "qa"]).success();
    at.run(&[
        "--session",
        "qa",
        "open",
        "--name",
        "app",
        "--size",
        "30x4",
        "--wait",
        "line 2",
        &fixture("line-printer"),
        "--lines",
        "2",
        "--hold",
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
    at.run_with_env(&["shell", "--name", "shell", "--shell", "bash", "--cwd", &cwd], &[("HOME", home.as_path())])
        .success();
    at.run(&["exec", "printf '\\033[32mgreen\\033[0m plain\\n'"]).success();
    let port = start(&at);

    let served = get(port, "/api/session/work/shell/commands/1", &[]);
    assert_eq!(served.status, 200, "{}", served.head);
    let mut served: serde_json::Value = serde_json::from_str(&served.body).expect("JSON");
    assert_eq!(served["output"], "\u{1b}[32mgreen\u{1b}[0m plain\n");
    let mut shown = at.run(&["--json", "history", "show", "1"]).success().json();
    assert_eq!(shown["output"], "green plain\n");
    served["output"] = serde_json::Value::Null;
    shown["output"] = serde_json::Value::Null;
    assert_eq!(served, shown);

    let missing = get(port, "/api/session/work/shell/commands/9", &[]);
    assert_eq!((missing.status, missing.body.as_str()), (404, "no command 9 in work shell"));
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
    a_foreign_host_and_a_foreign_post_are_refused(port);
}

fn a_foreign_host_and_a_foreign_post_are_refused(port: u16) {
    let rebound = exchange(port, "GET /api/sessions HTTP/1.1\r\nHost: evil.example\r\nConnection: close\r\n\r\n");
    assert_eq!(rebound.status, 403);
    let closing = exchange(
        port,
        &format!(
            "POST /api/session/qa/app/close HTTP/1.1\r\nHost: localhost:{port}\r\nOrigin: http://evil.example\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
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
    let port = u16::try_from(started["port"].as_u64().expect("the port")).expect("a port");

    assert_eq!(get(port, "/api/sessions", &[("Origin", "http://localhost:5173")]).status, 200);
    assert_eq!(get(port, "/api/sessions", &[("Origin", &format!("http://localhost:{port}"))]).status, 200);
    assert_eq!(get(port, "/api/sessions", &[("Origin", "http://localhost:5174")]).status, 403);
    assert_eq!(get(port, "/api/sessions", &[("Origin", "http://evil.example")]).status, 403);
    an_origin_without_a_scheme_is_refused_at_start(&at);
}

fn an_origin_without_a_scheme_is_refused_at_start(at: &AgentTerminal) {
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
    stopping_again_says_no_dashboard_runs(&at);
}

fn stopping_again_says_no_dashboard_runs(at: &AgentTerminal) {
    assert_eq!(at.run(&["dashboard", "stop"]).success().stdout(), "no dashboard is running\n");
    assert_eq!(at.run(&["--json", "dashboard", "stop"]).success().json(), serde_json::json!({ "stopped": false }));

    at.run(&["daemon", "stop"]).success();
    assert_eq!(at.run(&["dashboard", "stop"]).success().stdout(), "no dashboard is running\n");
}

#[test]
fn starting_again_on_any_port_succeeds_with_the_running_dashboard() {
    let at = AgentTerminal::new();
    let port = start(&at);
    let running = serde_json::json!({ "url": format!("http://localhost:{port}"), "port": port });

    assert_eq!(at.run(&["--json", "dashboard", "start", "--port", &port.to_string()]).success().json(), running);
    assert_eq!(at.run(&["--json", "dashboard", "--port", "0"]).success().json(), running);

    let free = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("find a free port");
    let other = free.local_addr().expect("its address").port().to_string();
    drop(free);
    let again = at.run(&["dashboard", "start", "--port", &other, "--allow-origin", "http://localhost:5173"]);
    assert_eq!(
        (again.code(), again.stdout(), again.stderr()),
        (0, format!("dashboard running at http://localhost:{port}\n"), String::new())
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
    at.run(&["open", "--name", "app", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    let png = at.home().join("evidence.png");
    std::fs::write(&png, b"\x89PNG\r\n\x1a\n").expect("write a PNG file");
    let log = at.home().join("sessions/work/activity.jsonl");
    let port = start(&at);

    for (session, file) in [("work", &png), ("work", &log), ("other", &png)] {
        let path = file.to_string_lossy();
        let refused = get(port, &format!("/api/session/{session}/screenshots/{}", path_segment(&path)), &[]);
        assert_eq!(refused.status, 404, "{}", refused.head);
        assert_eq!(refused.body, format!("{path} is not a PNG screenshot in session {session}"));
    }
}

#[test]
fn a_listed_screenshot_deleted_from_disk_says_it_cannot_be_read() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    let shot = at.home().join("gone.png");
    at.run(&["screenshot", &shot.to_string_lossy()]).success();
    std::fs::remove_file(&shot).expect("delete the screenshot");
    let port = start(&at);

    let path = shot.to_string_lossy();
    let missing = get(port, &format!("/api/session/work/screenshots/{}", path_segment(&path)), &[]);
    assert_eq!(missing.status, 404, "{}", missing.head);
    assert!(missing.body.starts_with(&format!("cannot read the screenshot {path}: ")), "{}", missing.body);
}

#[test]
fn opening_from_the_dashboard_without_home_needs_an_absolute_working_directory() {
    let at = AgentTerminal::unstarted();
    let started =
        at.command(&["--json", "dashboard", "start", "--port", "0"]).env_remove("HOME").output().expect("start");
    assert!(started.status.success(), "{}", String::from_utf8_lossy(&started.stderr));
    let started: serde_json::Value = serde_json::from_slice(&started.stdout).expect("JSON");
    let port = u16::try_from(started["port"].as_u64().expect("a port")).expect("a port number");
    let opening = |cwd: &str| {
        format!(
            r#"{{"session":"s","new_session":true,"tab":"cat","command":"cat","cwd":"{cwd}","cols":20,"rows":5,"shell_integration":false}}"#
        )
    };

    let refused = post(port, "/api/sessions", &opening("~/work"));
    assert_eq!(refused.status, 422, "{}", refused.body);
    assert!(
        refused.body.contains(r#"HOME is not set, so \"~/work\" has no home to start from; give an absolute path"#),
        "{}",
        refused.body
    );
    assert_eq!(post(port, "/api/sessions", &opening("/")).status, 200);
}

#[test]
fn a_working_directory_typed_in_the_dashboard_is_absolute_or_under_home() {
    let at = AgentTerminal::unstarted();
    let home = crate::support::scratch("home-");
    std::fs::create_dir_all(home.path().join("work/app")).expect("make folders under home");
    let started =
        at.command(&["--json", "dashboard", "start", "--port", "0"]).env("HOME", home.path()).output().expect("start");
    assert!(started.status.success(), "{}", String::from_utf8_lossy(&started.stderr));
    let started: serde_json::Value = serde_json::from_slice(&started.stdout).expect("JSON");
    let port = u16::try_from(started["port"].as_u64().expect("a port")).expect("a port number");

    let mut cwds = Vec::new();
    for (session, typed) in [("a", "work/app"), ("b", " ~/work "), ("c", "~"), ("d", "/")] {
        let opening = format!(
            r#"{{"session":"{session}","new_session":true,"tab":"cat","command":"cat","cwd":"{typed}","cols":20,"rows":5,"shell_integration":false}}"#
        );
        assert_eq!(post(port, "/api/sessions", &opening).status, 200, "{typed}");
        cwds.push(at.run(&["--session", session, "get", "cwd"]).success().stdout());
    }
    let home = home.path().display().to_string();
    assert_eq!(cwds, [format!("{home}/work/app\n"), format!("{home}/work\n"), format!("{home}\n"), "/\n".to_string()]);
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
    at.run(&["session", "new", "qa"]).success();
    at.run(&[
        "--session",
        "qa",
        "open",
        "--name",
        "app",
        "--size",
        "20x4",
        "--wait",
        "line 3",
        &fixture("line-printer"),
        "--hold",
    ])
    .success();
    at.run(&["--session", "qa", "wait", "--cursor", "4,1"]).success();
    let frame: u64 = at.run(&["--session", "qa", "get", "frame"]).success().stdout().trim().parse().unwrap();
    let port = start(&at);

    let taken = post(port, "/api/session/qa/app/screenshot", "");

    assert_eq!(taken.status, 200, "{}{}", taken.head, taken.body);
    let path = at.home().join("screenshots").join(format!("qa-app-{frame}.png")).to_string_lossy().into_owned();
    let reply: serde_json::Value = serde_json::from_str(&taken.body).expect("JSON");
    assert_eq!(reply["data"]["path"], path.as_str());
    the_screenshot_is_listed_without_a_call(&at, &path, frame);
    the_screenshot_is_served_and_not_logged_as_a_call(&at, port, &path);
}

fn the_screenshot_is_listed_without_a_call(at: &AgentTerminal, path: &str, frame: u64) {
    let listed = screenshots_listed(at, "qa");
    assert_eq!(listed.len(), 1);
    assert_eq!(
        (&listed[0]["path"], &listed[0]["by"], &listed[0]["frame"]),
        (&path.into(), &serde_json::Value::Null, &frame.into())
    );
}

fn the_screenshot_is_served_and_not_logged_as_a_call(at: &AgentTerminal, port: u16, path: &str) {
    let served = get(port, &screenshot_url("qa", path), &[]);
    assert_eq!(served.status, 200, "{}{}", served.head, served.body);
    assert_eq!(served.bytes, std::fs::read(path).expect("read the screenshot"));
    let activity = std::fs::read_to_string(at.home().join("sessions/qa/activity.jsonl")).expect("the activity log");
    assert!(!activity.contains(path), "{activity}");
}

fn compare(port: u16, a: &str, b: &str) -> serde_json::Value {
    let pair = serde_json::json!({ "paths": [a, b] }).to_string();
    let compared = post(port, "/api/session/work/screenshots/compare", &pair);
    match compared.status {
        200 => serde_json::from_str(&compared.body).expect("JSON"),
        status => serde_json::json!({ "status": status, "error": compared.body }),
    }
}

#[test]
fn the_dashboard_counts_and_boxes_the_cells_two_screenshots_differ_in() {
    let at = AgentTerminal::new();
    let shots = at.home().join("shots");
    let [before, after, bigger] =
        ["before.png", "after.png", "bigger.png"].map(|name| shots.join(name).to_string_lossy().into_owned());
    let lines = "stty -echo; printf '\\033[?25lone'; read _; printf '\\ntwo'; read _";
    at.run(&["open", "--name", "app", "--size", "20x4", "--wait", "one", "sh", "-c", lines]).success();
    at.run(&["screenshot", &before]).success();
    at.run(&["press", "Enter", "--wait", "two", "--screenshot", &after]).success();
    at.run(&["screenshot", "--scale", "2", &bigger]).success();
    let port = start(&at);

    let three = serde_json::json!({ "changed": 3, "boxes": [{ "x": 10, "y": 30, "width": 27, "height": 20 }] });
    assert_eq!(compare(port, &before, &after), three);
    assert_eq!(compare(port, &after, &before), three);
    assert_eq!(compare(port, &before, &before), serde_json::json!({ "changed": 0, "boxes": [] }));
    assert_eq!(compare(port, &before, &bigger), serde_json::json!({ "changed": null, "boxes": [] }));
    a_file_outside_the_session_or_a_resized_one_is_not_compared(&at, port, &before, &after);
}

fn a_file_outside_the_session_or_a_resized_one_is_not_compared(
    at: &AgentTerminal,
    port: u16,
    before: &str,
    after: &str,
) {
    let shots = at.home().join("shots");
    let elsewhere = shots.join("elsewhere.png").to_string_lossy().into_owned();
    let refusal = format!("{elsewhere} is not a PNG screenshot in session work");
    assert_eq!(compare(port, before, &elsewhere), serde_json::json!({ "status": 404, "error": refusal }));

    at.run(&["screenshot", "--scale", "2", before]).success();
    assert_eq!(compare(port, before, after), serde_json::json!({ "changed": null, "boxes": [] }));
}
