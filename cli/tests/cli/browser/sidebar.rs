use super::{Browser, Dashboard, snapshot_rows};
use crate::support::{AgentTerminal, fixture};

fn sidebar(browser: &Browser) -> Vec<String> {
    browser
        .snapshot("nav")
        .lines()
        .filter_map(|line| {
            let line = line.trim().strip_prefix("- ")?;
            let named = line.split('"').nth(1)?;
            (line.starts_with("button") || line.starts_with("link")).then(|| named.to_string())
        })
        .collect()
}

#[test]
fn the_sidebar_groups_tabs_by_session_with_program_and_status() {
    let at = AgentTerminal::without_session();
    open_qa_with_two_tabs_and_build_exited(&at);
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/"));

    the_sidebar_lists_each_session_with_its_tabs(&browser);
    a_tab_link_opens_that_terminal(&at, &browser);
    the_sidebar_follows_new_sessions_and_closed_tabs(&at, &browser);
}

fn open_qa_with_two_tabs_and_build_exited(at: &AgentTerminal) {
    for session in ["qa", "build"] {
        at.run(&["session", "new", session]).success();
    }
    at.run(&["--session", "qa", "open", "--name", "app", "--size", "50x8", "--wait", "Files", &fixture("fixture-tui")])
        .success();
    at.run(&[
        "--session",
        "qa",
        "tab",
        "new",
        "--name",
        "api",
        "--size",
        "30x4",
        "--wait",
        "line 1",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--hold",
    ])
    .success();
    at.run(&[
        "--session",
        "build",
        "open",
        "--name",
        "app",
        "--size",
        "30x4",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--exit",
        "3",
    ])
    .success();
}

fn the_sidebar_lists_each_session_with_its_tabs(browser: &Browser) {
    browser.wait_until("location.pathname === '/s/build/app'");
    browser.wait_for_text("exited 3");
    assert_eq!(
        sidebar(browser),
        [
            "build 1 , 1 tab",
            "app line-printer , build app, exited 3 exited 3",
            "New tab in build",
            "qa 2 , 2 tabs",
            "app fixture-tui , qa app, running",
            "api line-printer , qa api, running",
            "New tab in qa",
        ]
    );
}

fn a_tab_link_opens_that_terminal(at: &AgentTerminal, browser: &Browser) {
    browser.run(&["find", "role", "link", "click", "--name", "app fixture-tui , qa app, running", "--exact"]);
    browser.wait_until("location.pathname === '/s/qa/app'");
    browser.wait_for_screen("Move: j/k");
    browser.wait_for_status("alt-screen");
    assert_eq!(browser.screen(), snapshot_rows(at, &["--session", "qa", "--tab", "app"]).0);
}

fn the_sidebar_follows_new_sessions_and_closed_tabs(at: &AgentTerminal, browser: &Browser) {
    at.run(&["session", "new", "chat"]).success();
    at.run(&[
        "--session",
        "chat",
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
    browser.wait_for_text("chat");
    at.run(&["--session", "qa", "tab", "close", "api"]).success();
    browser.wait_until("!document.querySelector('nav').innerText.includes('qa api')");
    assert_eq!(
        sidebar(browser),
        [
            "build 1 , 1 tab",
            "app line-printer , build app, exited 3 exited 3",
            "New tab in build",
            "chat 1 , 1 tab",
            "app line-printer , chat app, running",
            "New tab in chat",
            "qa 1 , 1 tab",
            "app fixture-tui , qa app, running",
            "New tab in qa",
        ]
    );
}

#[test]
fn a_tab_that_is_not_open_says_so_and_links_back() {
    let at = AgentTerminal::without_session();
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
        "line 1",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--hold",
    ])
    .success();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/qa/gone"));

    browser.wait_for_text("qa gone is not open");
    assert_eq!(browser.run(&["get", "title"]).trim(), "Not open · agent-terminal");
    browser.run(&["find", "role", "link", "click", "--name", "Show open sessions", "--exact"]);
    browser.wait_until("location.pathname === '/s/qa/app'");
}

const HANDLE: &str = "[role=separator][aria-label='Resize the side panel']";

fn panel_width(browser: &Browser) -> String {
    browser.eval(&format!("document.querySelector(\"{HANDLE}\").ariaValueNow"))
}

#[test]
fn the_side_panel_keeps_its_tab_and_width_from_one_terminal_to_the_next() {
    let at = AgentTerminal::without_session();
    at.run(&["session", "new", "qa"]).success();
    for verb in [&["open"][..], &["tab", "new"][..]] {
        let printer = fixture("line-printer");
        let mut args = vec!["--session", "qa"];
        args.extend(verb);
        args.extend(["--size", "30x4", "--wait", "line 1", printer.as_str(), "--lines", "1", "--hold"]);
        at.run(&args).success();
    }
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/qa/app"));
    browser.wait_for_status("Live");
    assert_eq!(panel_width(&browser), "\"30\"");

    browser.select_panel("Output");
    browser.run(&["focus", HANDLE]);
    browser.run(&["press", "ArrowLeft"]);
    assert_eq!(panel_width(&browser), "\"32\"");
    the_next_terminal_keeps_the_output_panel_at_that_width(&browser);
}

fn the_next_terminal_keeps_the_output_panel_at_that_width(browser: &Browser) {
    browser.run(&["find", "role", "link", "click", "--name", "api line-printer , qa api, running", "--exact"]);
    browser.wait_until("location.pathname === '/s/qa/api'");
    browser.wait_until("document.querySelector('[aria-label=\"Details of qa api\"]') !== null");
    assert_eq!(browser.eval("document.querySelector('[role=tab][aria-selected=true]').textContent"), "\"Output\"");
    assert_eq!(panel_width(browser), "\"32\"");
}
