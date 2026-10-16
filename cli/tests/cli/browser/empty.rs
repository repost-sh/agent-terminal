use super::{Browser, Dashboard};
use crate::support::{AgentTerminal, fixture};

#[test]
fn with_no_sessions_the_dashboard_offers_the_commands_to_get_started() {
    let at = AgentTerminal::without_session();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/"));

    browser.wait_for_text("No active sessions");
    assert_eq!(browser.run(&["get", "title"]).trim(), "No active sessions · agent-terminal");
    assert_eq!(
        browser.text("main").lines().filter(|line| !line.trim().is_empty()).collect::<Vec<_>>(),
        [
            "No active sessions",
            "No active sessions",
            "Start a terminal from the CLI, or create a session here",
            "agent-terminal session new <name>",
            "Copy agent-terminal session new <name>",
            "agent-terminal --session <name> open --name <tab> <program>",
            "Copy agent-terminal --session <name> open --name <tab> <program>",
            "agent-terminal --session <name> shell --name <tab>",
            "Copy agent-terminal --session <name> shell --name <tab>",
            "agent-terminal skills get core",
            "Copy agent-terminal skills get core",
            "New session",
        ]
    );
    assert_eq!(browser.text("nav").trim(), "No sessions");
}

#[test]
fn with_sessions_but_no_tabs_the_dashboard_offers_to_open_a_tab_in_each() {
    let at = AgentTerminal::new();
    at.run(&["session", "new", "spare"]).success();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/"));

    browser.wait_for_text("No open tabs");
    assert_eq!(browser.run(&["get", "title"]).trim(), "No open tabs · agent-terminal");
    assert_eq!(
        browser.text("main").lines().filter(|line| !line.trim().is_empty()).collect::<Vec<_>>(),
        [
            "No open tabs",
            "No open tabs",
            "Open a tab in spare and work from the CLI, or here",
            "agent-terminal --session <name> open --name <tab> <program>",
            "Copy agent-terminal --session <name> open --name <tab> <program>",
            "agent-terminal --session <name> shell --name <tab>",
            "Copy agent-terminal --session <name> shell --name <tab>",
            "agent-terminal skills get core",
            "Copy agent-terminal skills get core",
            "New tab in spare",
            "New tab in work",
        ]
    );

    browser.click_button_in("main", "New tab in spare");
    browser.wait_until("document.querySelector('[role=dialog] h2')?.textContent === 'New tab'");
    assert_eq!(browser.run(&["get", "value", "[role=dialog] select"]).trim(), "spare");
}

#[test]
fn losing_the_daemon_shows_reconnecting_then_disconnected_and_retry_reconnects() {
    let at = AgentTerminal::new();
    at.run(&[
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
    let browser = Browser::open(&dashboard.url("/s/work/app"));
    browser.wait_for_status("Live");

    at.run(&["daemon", "stop"]).success();

    browser.wait_for_status("Reconnecting…");
    browser.wait_for_text("Disconnected");
    assert_eq!(browser.run(&["get", "title"]).trim(), "Disconnected · agent-terminal");
    let port = dashboard.port;
    let start = format!("agent-terminal dashboard start --port {port}");
    assert_eq!(
        browser.text("main").lines().filter(|line| !line.trim().is_empty()).collect::<Vec<_>>(),
        [
            "Disconnected".to_string(),
            "Disconnected".to_string(),
            format!("The agent-terminal daemon is not running on :{port}"),
            start.clone(),
            format!("Copy {start}"),
            "Retry".to_string(),
        ]
    );
    assert_eq!(browser.text("nav").trim(), "Not connected");

    at.run(&["dashboard", "start", "--port", &port.to_string()]).success();
    browser.click_button("Retry");
    browser.wait_for_text("No active sessions");
    browser.wait_for_announcement("Connected");
}

#[test]
fn the_dashboard_starts_dark_and_switches_to_light_and_back() {
    let at = AgentTerminal::without_session();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/"));
    browser.wait_for_text("No active sessions");
    let background = || browser.eval("getComputedStyle(document.body).backgroundColor");

    assert_eq!(background(), "\"rgb(10, 10, 10)\"");
    browser.click_button("Switch to light mode");
    browser.wait_until("!document.documentElement.classList.contains('dark')");
    assert_eq!(background(), "\"rgb(255, 255, 255)\"");

    browser.run(&["reload"]);
    browser.wait_for_text("No active sessions");
    assert_eq!(background(), "\"rgb(255, 255, 255)\"");
    browser.click_button("Switch to dark mode");
    browser.wait_until("document.documentElement.classList.contains('dark')");
    assert_eq!(background(), "\"rgb(10, 10, 10)\"");
}
