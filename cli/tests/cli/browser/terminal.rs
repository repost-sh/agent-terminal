use super::{Browser, Dashboard, snapshot_rows};
use crate::support::{AgentTerminal, Cue, fixture};

fn stream(dashboard: &Dashboard) -> String {
    format!("ws://localhost:{}/api/session/default/t1/stream", dashboard.port)
}

#[test]
fn the_live_terminal_shows_the_screen_on_connect_then_streams_matching_snapshot() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    at.run(&[
        "open",
        "--size",
        "40x6",
        "--wait",
        "line 2",
        &fixture("line-printer"),
        "--lines",
        "2",
        "--cue",
        &cue.path(),
    ])
    .success();
    cue.print("");
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));

    let (rows, frame) = snapshot_rows(&at, &[]);
    browser.wait_for_status(&format!("frame {frame}"));
    browser.wait_for_screen("line 2");
    assert_eq!(browser.screen(), rows);
    assert_eq!(browser.status(), ["Live", &stream(&dashboard), "40 × 6", &format!("frame {frame}")]);
    assert_eq!(browser.run(&["get", "title"]).trim(), "default t1 · line-printer · agent-terminal");

    cue.print("streamed\n");
    let (rows, frame) = snapshot_rows(&at, &[]);
    assert_eq!(rows, ["line 1", "line 2", "streamed"]);
    browser.wait_for_status(&format!("frame {frame}"));
    browser.wait_for_screen("streamed");
    assert_eq!(browser.screen(), rows);

    cue.print("\x1b[?2004h\x1b[?1000h\x1b[?1006h");
    let (_, frame) = snapshot_rows(&at, &[]);
    browser.wait_for_status("bracketed paste");
    assert_eq!(
        browser.status(),
        ["Live", &stream(&dashboard), "40 × 6", "mouse sgr", "bracketed paste", &format!("frame {frame}")]
    );
}

#[test]
fn a_tab_that_has_not_drawn_yet_shows_connected_until_its_first_frame() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    at.run(&["open", "--size", "40x6", &fixture("line-printer"), "--lines", "0", "--cue", &cue.path()]).success();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));

    browser.wait_for_text("Waiting for first frame...");
    browser.wait_for_status("Connected");
    assert_eq!(browser.status(), ["Connected", &stream(&dashboard), "40 × 6"]);

    cue.print("first\n");
    browser.wait_for_status("Live");
    browser.wait_for_screen("first");
    assert!(!browser.text("main").contains("Waiting for first frame..."));
}

#[test]
fn an_exited_tab_keeps_its_last_screen_with_the_exit_bar_to_restart_or_close_it() {
    let at = AgentTerminal::new();
    at.run(&[
        "open",
        "--size",
        "40x6",
        "--wait",
        "line 2",
        &fixture("line-printer"),
        "--lines",
        "2",
        "--hold",
        "--exit",
        "4",
    ])
    .success();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    browser.wait_for_status("Live");
    browser.wait_for_screen("line 2");

    at.run(&["press", "Enter"]).success();
    browser.wait_for_status("Exited");
    browser.wait_for_text("exited with code");
    let (rows, frame) = snapshot_rows(&at, &[]);
    assert_eq!(&rows[..2], ["line 1", "line 2"]);
    assert_eq!(browser.screen(), rows);
    assert_eq!(browser.status(), ["Exited", &stream(&dashboard), "40 × 6", &format!("frame {frame}")]);
    assert_eq!(
        browser.text("main [role=status]").lines().collect::<Vec<_>>(),
        ["line-printer exited with code 4", "Restart", "Close tab"]
    );
    assert!(browser.text("nav").contains("exited 4"), "{}", browser.text("nav"));

    browser.click_button("Restart");
    browser.wait_for_status("Live");
    browser.wait_until("document.activeElement?.id === 'take-control-default-t1'");
    browser.wait_for_announcement("line-printer restarted");
    assert_eq!(at.run(&["get", "exit-code"]).success().stdout(), "running\n");
    assert!(!browser.text("main").contains("exited with code"));
    browser.wait_for_screen("line 2");

    at.run(&["press", "Enter"]).success();
    browser.wait_for_status("Exited");
    browser.click_button("Close tab");
    browser.wait_for_text("No active sessions");
    browser.wait_until("location.pathname === '/'");
    assert_eq!(at.run(&["session", "list"]).success().stdout(), "no sessions are open\n");
}
