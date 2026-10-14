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
    let at = AgentTerminal::new();
    at.run(&["--session", "qa", "open", "--size", "50x8", "--wait", "Files", &fixture("fixture-tui")]).success();
    at.run(&[
        "--session",
        "qa",
        "tab",
        "new",
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
    at.run(&["--session", "build", "open", "--size", "30x4", &fixture("line-printer"), "--lines", "1", "--exit", "3"])
        .success();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/"));

    browser.wait_until("location.pathname === '/s/build/t1'");
    browser.wait_for_text("exited 3");
    assert_eq!(
        sidebar(&browser),
        [
            "build 1 , 1 tab",
            "line-printer , build t1, exited 3 exited 3",
            "New tab in build",
            "qa 2 , 2 tabs",
            "fixture-tui , qa t1, running",
            "line-printer , qa t2, running",
            "New tab in qa",
        ]
    );

    browser.run(&["find", "role", "link", "click", "--name", "fixture-tui , qa t1, running", "--exact"]);
    browser.wait_until("location.pathname === '/s/qa/t1'");
    browser.wait_for_screen("Move: j/k");
    browser.wait_for_status("alt-screen");
    assert_eq!(browser.screen(), snapshot_rows(&at, &["--session", "qa", "--tab", "t1"]).0);

    at.run(&[
        "--session",
        "chat",
        "open",
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
    at.run(&["--session", "qa", "tab", "close", "t2"]).success();
    browser.wait_until("!document.querySelector('nav').innerText.includes('qa t2')");
    assert_eq!(
        sidebar(&browser),
        [
            "build 1 , 1 tab",
            "line-printer , build t1, exited 3 exited 3",
            "New tab in build",
            "chat 1 , 1 tab",
            "line-printer , chat t1, running",
            "New tab in chat",
            "qa 1 , 1 tab",
            "fixture-tui , qa t1, running",
            "New tab in qa",
        ]
    );
}

#[test]
fn a_tab_that_is_not_open_says_so_and_links_back() {
    let at = AgentTerminal::new();
    at.run(&[
        "--session",
        "qa",
        "open",
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
    let browser = Browser::open(&dashboard.url("/s/qa/t9"));

    browser.wait_for_text("qa t9 is not open");
    assert_eq!(browser.run(&["get", "title"]).trim(), "Not open · agent-terminal");
    browser.run(&["find", "role", "link", "click", "--name", "Show open sessions", "--exact"]);
    browser.wait_until("location.pathname === '/s/qa/t1'");
}
