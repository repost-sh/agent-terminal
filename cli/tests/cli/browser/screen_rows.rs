use super::{Browser, Dashboard};
use crate::support::AgentTerminal;

const LISTED_ROWS: &str = "document.querySelectorAll('main [aria-label=\"Screen\"] > *').length";

fn open_echo(at: &AgentTerminal) -> Browser {
    at.run(&[
        "open",
        "--name",
        "app",
        "--size",
        "20x4",
        "--wait",
        "ready",
        "--",
        "sh",
        "-c",
        "printf \"ready\\n\"; cat",
    ])
    .success();
    let dashboard = Dashboard::start(at);
    Browser::open(&dashboard.url("/s/work/app"))
}

#[test]
fn the_screen_is_a_list_of_rows_that_follows_every_frame() {
    let at = AgentTerminal::new();
    let browser = open_echo(&at);

    browser.wait_for_screen("ready");
    assert_eq!(browser.screen(), ["ready"]);

    at.run(&["type", "hello"]).success();
    browser.wait_for_screen("hello");
    assert_eq!(browser.screen(), ["ready", "hello"]);
    browser.wait_until(&format!("{LISTED_ROWS} === 4"));

    at.run(&["resize", "30x6"]).success();
    browser.wait_until(&format!("{LISTED_ROWS} === 6"));
}

#[test]
fn xterm_has_no_live_region_and_no_accessibility_tree() {
    let at = AgentTerminal::new();
    let browser = open_echo(&at);

    browser.wait_for_screen("ready");
    browser.wait_until(
        "document.querySelector('.live-region') === null && document.querySelector('.xterm-accessibility-tree') === null",
    );
}
