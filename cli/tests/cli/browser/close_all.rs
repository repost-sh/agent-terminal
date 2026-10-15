use super::{Browser, Dashboard};
use crate::support::{AgentTerminal, fixture};

fn hold(at: &AgentTerminal, session: &str, verb: &[&str]) {
    let mut args = vec!["--session", session];
    args.extend_from_slice(verb);
    let printer = fixture("line-printer");
    args.extend_from_slice(&["--size", "30x4", "--wait", "line 1", &printer, "--lines", "1", "--hold"]);
    at.run(&args).success();
}

fn alert(browser: &Browser) -> Vec<String> {
    browser
        .text("[role=alertdialog]")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn focused(browser: &Browser) -> String {
    browser.eval("document.activeElement.textContent")
}

const ALERT_OPEN: &str = "document.querySelector('[role=alertdialog]') !== null";
const ALERT_CLOSED: &str = "document.querySelector('[role=alertdialog]') === null";

#[test]
fn close_all_asks_first_naming_the_sessions_and_running_processes_then_closes_everything() {
    let at = AgentTerminal::new();
    open_two_sessions_with_a_running_and_an_exited_tab(&at);
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/"));
    browser.wait_until("location.pathname === '/s/build/t1'");
    browser.wait_for_text("exited 3");

    the_question_names_the_sessions_and_escape_dismisses_it(&browser);
    cancel_closes_nothing(&at, &browser);
    close_all_closes_every_session(&at, &browser);
    the_question_counts_one_session_in_the_singular(&at, &browser);
}

fn open_two_sessions_with_a_running_and_an_exited_tab(at: &AgentTerminal) {
    hold(at, "qa", &["open"]);
    at.run(&[
        "--session",
        "qa",
        "tab",
        "new",
        "--size",
        "30x4",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--exit",
        "3",
    ])
    .success();
    at.run(&["--session", "qa", "--tab", "t2", "wait", "--exit"]).success();
    hold(at, "build", &["open"]);
}

fn the_question_names_the_sessions_and_escape_dismisses_it(browser: &Browser) {
    browser.click_button("Close all sessions");
    browser.wait_until(ALERT_OPEN);
    assert_eq!(
        alert(browser),
        [
            "Close all sessions",
            "This will close 2 sessions and kill 2 running processes. This action cannot be undone.",
            "Cancel",
            "Close all",
        ]
    );
    assert_eq!(focused(browser), "\"Cancel\"");
    browser.run(&["press", "Escape"]);
    browser.wait_until(ALERT_CLOSED);
    assert_eq!(focused(browser), "\"Close all sessions\"");
}

fn cancel_closes_nothing(at: &AgentTerminal, browser: &Browser) {
    browser.click_button("Close all sessions");
    browser.wait_until(ALERT_OPEN);
    browser.click_button("Cancel");
    browser.wait_until(ALERT_CLOSED);
    assert_eq!(at.run(&["session", "list"]).success().stdout().lines().count(), 3);
}

fn close_all_closes_every_session(at: &AgentTerminal, browser: &Browser) {
    browser.click_button("Close all sessions");
    browser.wait_until(ALERT_OPEN);
    browser.click_button("Close all");
    browser.wait_for_text("No active sessions");
    browser.wait_until("location.pathname === '/'");
    assert_eq!(at.run(&["session", "list"]).success().stdout(), "no sessions are open\n");
    assert_eq!(browser.run(&["is", "enabled", "button[data-tooltip='Close all sessions']"]).trim(), "false");
}

fn the_question_counts_one_session_in_the_singular(at: &AgentTerminal, browser: &Browser) {
    hold(at, "solo", &["open"]);
    browser.wait_until("location.pathname === '/s/solo/t1'");
    browser.click_button("Close all sessions");
    browser.wait_until(ALERT_OPEN);
    assert_eq!(
        alert(browser)[1],
        "This will close 1 session and kill 1 running process. This action cannot be undone."
    );
}
