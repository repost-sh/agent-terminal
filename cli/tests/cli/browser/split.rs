use super::{Browser, Dashboard, snapshot_rows, timeline_controls};
use crate::support::{AgentTerminal, Cue, fixture};

fn hold(at: &AgentTerminal, opening: &[&str], size: &str, lines: &str) {
    let last = format!("line {lines}");
    let printer = fixture("line-printer");
    let mut call = opening.to_vec();
    call.extend_from_slice(&["--size", size, "--wait", &last, &printer, "--lines", lines, "--hold"]);
    at.run(&call).success();
}

fn grid(browser: &Browser) -> Vec<String> {
    browser
        .snapshot(".split-grid")
        .lines()
        .filter_map(|line| {
            let line = line.trim().strip_prefix("- ")?;
            ["region", "button", "link", "heading"]
                .iter()
                .any(|role| line.starts_with(&format!("{role} ")))
                .then(|| line.split(" [").next().unwrap_or(line).to_string())
        })
        .collect()
}

fn layout(browser: &Browser) -> String {
    browser.eval(
        "document.querySelector('main [role=group][aria-label=Layout] [aria-pressed=true]')?.textContent ?? 'none'",
    )
}

fn focused(browser: &Browser) -> String {
    browser
        .eval("document.querySelector('.split-grid button[aria-pressed=true]')?.closest('section').ariaLabel ?? 'none'")
}

fn keyboard_focus(browser: &Browser) -> String {
    browser.eval("document.activeElement.textContent")
}

fn selected_in_sidebar(browser: &Browser) -> String {
    browser.eval("document.querySelector('nav [aria-current=page]')?.textContent ?? 'none'")
}

fn stream(dashboard: &Dashboard, tab: &str) -> String {
    format!("ws://localhost:{}/api/session/{tab}/stream", dashboard.port)
}

fn cell(name: &str) -> String {
    format!("main section[aria-label={name:?}]")
}

#[test]
fn two_up_from_a_tab_shows_it_beside_the_tab_after_it_both_live() {
    let at = AgentTerminal::new();
    hold(&at, &["--session", "qa", "open"], "30x4", "1");
    let cue = Cue::new();
    at.run(&[
        "--session",
        "qa",
        "tab",
        "new",
        "--size",
        "32x5",
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
    hold(&at, &["--session", "web", "open"], "30x4", "3");
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/qa/t1"));
    browser.wait_for_status("Live");

    browser.click_button("Two terminals side by side");
    browser.wait_until("location.pathname === '/split/qa:t1,qa:t2'");
    assert_eq!(browser.run(&["get", "title"]).trim(), "Split · 2 terminals · agent-terminal");
    assert_eq!(
        grid(&browser),
        [
            "region \"qa t1 line-printer\"",
            "button \"qa t1 line-printer , running\"",
            "link \"Maximize qa t1\"",
            "button \"Remove qa t1 from the grid\"",
            "region \"qa t2 line-printer\"",
            "button \"qa t2 line-printer , running\"",
            "link \"Maximize qa t2\"",
            "button \"Remove qa t2 from the grid\"",
        ]
    );
    assert_eq!(layout(&browser), "\"Two terminals side by side\"");
    assert_eq!(keyboard_focus(&browser), "\"Two terminals side by side\"");
    assert_eq!(focused(&browser), "\"qa t1 line-printer\"");
    browser.wait_for_screen_in(&cell("qa t1 line-printer"), "line 1");
    browser.wait_for_screen_in(&cell("qa t2 line-printer"), "line 2");
    assert_eq!(
        browser.screen_in(&cell("qa t1 line-printer")),
        snapshot_rows(&at, &["--session", "qa", "--tab", "t1"]).0
    );
    assert_eq!(
        browser.screen_in(&cell("qa t2 line-printer")),
        snapshot_rows(&at, &["--session", "qa", "--tab", "t2"]).0
    );

    cue.print("streamed\n");
    browser.wait_for_screen_in(&cell("qa t2 line-printer"), "streamed");
    assert_eq!(browser.screen_in(&cell("qa t2 line-printer")), ["line 1", "line 2", "streamed"]);

    let (_, frame) = snapshot_rows(&at, &["--session", "qa", "--tab", "t1"]);
    browser.wait_for_status(&format!("frame {frame}"));
    assert_eq!(browser.status(), ["Live", &stream(&dashboard, "qa/t1"), "30 × 4", &format!("frame {frame}")]);
    assert_eq!(timeline_controls(&browser), Vec::<String>::new());
}

#[test]
fn four_up_fills_with_the_current_tab_then_the_ones_after_it_and_single_returns_to_the_focused_tab() {
    let at = AgentTerminal::new();
    at.run(&["--session", "build", "open", "--size", "30x4", &fixture("line-printer"), "--lines", "1", "--exit", "3"])
        .success();
    hold(&at, &["--session", "chat", "open"], "30x4", "2");
    hold(&at, &["--session", "qa", "open"], "30x4", "1");
    hold(&at, &["--session", "qa", "tab", "new"], "30x4", "3");
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/chat/t1"));
    browser.wait_for_status("Live");

    browser.click_button("Four terminals");
    browser.wait_until("location.pathname === '/split/chat:t1,qa:t1,qa:t2,build:t1'");
    assert_eq!(browser.run(&["get", "title"]).trim(), "Split · 4 terminals · agent-terminal");
    assert_eq!(layout(&browser), "\"Four terminals\"");
    assert_eq!(focused(&browser), "\"chat t1 line-printer\"");
    browser.wait_for_text("exited with code");
    assert_eq!(
        grid(&browser),
        [
            "region \"chat t1 line-printer\"",
            "button \"chat t1 line-printer , running\"",
            "link \"Maximize chat t1\"",
            "button \"Remove chat t1 from the grid\"",
            "region \"qa t1 line-printer\"",
            "button \"qa t1 line-printer , running\"",
            "link \"Maximize qa t1\"",
            "button \"Remove qa t1 from the grid\"",
            "region \"qa t2 line-printer\"",
            "button \"qa t2 line-printer , running\"",
            "link \"Maximize qa t2\"",
            "button \"Remove qa t2 from the grid\"",
            "region \"build t1 line-printer\"",
            "button \"build t1 line-printer exited 3 , exited 3\"",
            "link \"Maximize build t1\"",
            "button \"Remove build t1 from the grid\"",
        ]
    );
    assert_eq!(
        browser.text(&format!("{} [role=status]", cell("build t1 line-printer"))).trim(),
        "line-printer exited with code 3"
    );
    browser.wait_for_screen_in(&cell("qa t2 line-printer"), "line 3");
    assert_eq!(browser.screen_in(&cell("qa t2 line-printer")), ["line 1", "line 2", "line 3"]);
    assert_eq!(browser.screen_in(&cell("build t1 line-printer")), ["line 1"]);

    browser.click_button("Two terminals side by side");
    browser.wait_until("location.pathname === '/split/chat:t1,qa:t1'");
    assert_eq!(focused(&browser), "\"chat t1 line-printer\"");

    browser.click_button("Single terminal");
    browser.wait_until("location.pathname === '/s/chat/t1'");
    browser.wait_for_status("Live");
    assert_eq!(layout(&browser), "\"Single terminal\"");
    assert_eq!(keyboard_focus(&browser), "\"Single terminal\"");
}

#[test]
fn removing_a_cell_offers_the_picker_and_picking_a_tab_fills_and_focuses_that_cell() {
    let at = AgentTerminal::new();
    hold(&at, &["--session", "qa", "open"], "30x4", "1");
    hold(&at, &["--session", "qa", "tab", "new"], "32x5", "2");
    hold(&at, &["--session", "web", "open"], "34x6", "3");
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/split/qa:t1,qa:t2"));
    browser.wait_for_status("Live");

    browser.click_button("Remove qa t2 from the grid");
    browser.wait_until("location.pathname === '/split/qa:t1,_'");
    assert_eq!(
        grid(&browser),
        [
            "region \"qa t1 line-printer\"",
            "button \"qa t1 line-printer , running\"",
            "link \"Maximize qa t1\"",
            "button \"Remove qa t1 from the grid\"",
            "region \"Show a terminal\"",
            "heading \"Show a terminal\"",
            "button \"t1 line-printer shown , qa t1\"",
            "button \"t2 line-printer , qa t2\"",
            "button \"t1 line-printer , web t1\"",
        ]
    );
    assert_eq!(
        browser.eval(
            "[...document.querySelectorAll('.split-grid section[aria-labelledby] button')].map((button) => button.disabled).join()"
        ),
        "\"true,false,false\""
    );
    assert_eq!(focused(&browser), "\"qa t1 line-printer\"");
    assert_eq!(browser.eval("document.activeElement.id"), "\"main-content\"");

    browser.click_button("t1 line-printer , web t1");
    browser.wait_until("location.pathname === '/split/qa:t1,web:t1'");
    assert_eq!(focused(&browser), "\"web t1 line-printer\"");
    assert_eq!(
        browser.eval("document.activeElement.closest('section')?.ariaLabel ?? 'none'"),
        "\"web t1 line-printer\""
    );
    let (_, frame) = snapshot_rows(&at, &["--session", "web"]);
    browser.wait_for_status(&format!("frame {frame}"));
    browser.wait_for_screen_in(&cell("web t1 line-printer"), "line 3");
    assert_eq!(browser.status(), ["Live", &stream(&dashboard, "web/t1"), "34 × 6", &format!("frame {frame}")]);

    browser.click_button("Remove web t1 from the grid");
    browser.wait_until("location.pathname === '/split/qa:t1,_'");
    assert_eq!(focused(&browser), "\"qa t1 line-printer\"");
    browser.wait_for_status("30 × 4");
}

#[test]
fn clicking_a_cell_focuses_it_and_a_tab_picked_in_the_sidebar_replaces_the_focused_cell() {
    let at = AgentTerminal::new();
    hold(&at, &["--session", "qa", "open"], "30x4", "1");
    hold(&at, &["--session", "qa", "tab", "new"], "32x5", "2");
    hold(&at, &["--session", "web", "open"], "34x6", "3");
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/split/qa:t1,qa:t2"));
    browser.wait_for_status("30 × 4");
    assert_eq!(selected_in_sidebar(&browser), "\"line-printer, qa t1, running\"");

    browser.wait_for_screen_in(&cell("qa t2 line-printer"), "line 2");
    browser.run(&["click", &format!("{} [role=group]", cell("qa t2 line-printer"))]);
    let (_, frame) = snapshot_rows(&at, &["--session", "qa", "--tab", "t2"]);
    browser.wait_for_status("32 × 5");
    browser.wait_for_status(&format!("frame {frame}"));
    assert_eq!(focused(&browser), "\"qa t2 line-printer\"");
    assert_eq!(browser.status(), ["Live", &stream(&dashboard, "qa/t2"), "32 × 5", &format!("frame {frame}")]);
    assert_eq!(selected_in_sidebar(&browser), "\"line-printer, qa t2, running\"");
    assert_eq!(browser.eval("location.pathname"), "\"/split/qa:t1,qa:t2\"");

    browser.run(&["find", "role", "link", "click", "--name", "line-printer , web t1, running", "--exact"]);
    browser.wait_until("location.pathname === '/split/qa:t1,web:t1'");
    assert_eq!(focused(&browser), "\"web t1 line-printer\"");
    browser.wait_for_status("34 × 6");
    browser.wait_for_screen_in(&cell("web t1 line-printer"), "line 3");
    assert_eq!(browser.screen_in(&cell("web t1 line-printer")), snapshot_rows(&at, &["--session", "web"]).0);
}

#[test]
fn maximize_shows_the_tab_alone_and_a_split_route_survives_a_reload() {
    let at = AgentTerminal::new();
    hold(&at, &["--session", "qa", "open"], "30x4", "1");
    hold(&at, &["--session", "web.app", "open"], "32x5", "2");
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/qa/t1"));
    browser.wait_for_status("Live");

    browser.click_button("Two terminals side by side");
    browser.wait_until("location.pathname === '/split/qa:t1,web%2Eapp:t1'");
    browser.run(&["reload"]);
    browser.wait_for_screen_in(&cell("web.app t1 line-printer"), "line 2");
    assert_eq!(focused(&browser), "\"qa t1 line-printer\"");

    browser.run(&["find", "role", "link", "click", "--name", "Maximize web.app t1", "--exact"]);
    browser.wait_until("location.pathname === '/s/web.app/t1'");
    assert_eq!(browser.eval("document.activeElement.id"), "\"main-content\"");
    browser.wait_for_status("32 × 5");
    browser.wait_for_screen("line 2");
    assert_eq!(layout(&browser), "\"Single terminal\"");
    assert_eq!(browser.run(&["get", "title"]).trim(), "web.app t1 · line-printer · agent-terminal");
}

#[test]
fn a_split_route_with_three_cells_is_not_a_page() {
    let at = AgentTerminal::new();
    hold(&at, &["--session", "qa", "open"], "30x4", "1");
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/split/qa:t1,_,_"));

    browser.wait_for_text("This page could not be found.");
    assert_eq!(browser.run(&["get", "title"]).trim(), "404: This page could not be found · agent-terminal");
}

#[test]
fn the_focused_cell_of_a_split_takes_the_keys_of_a_human_in_control() {
    let at = AgentTerminal::new();
    at.run(&["--session", "app", "open", "--size", "40x6", "--wait", "ready", &fixture("input-echo")]).success();
    hold(&at, &["--session", "web", "open"], "30x4", "1");
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/split/app:t1,web:t1"));
    browser.wait_for_status("Live");
    browser.wait_for_screen_in(&cell("app t1 input-echo"), "ready");

    browser.click_button("Take control");
    browser.wait_until("document.activeElement?.getAttribute('aria-label') === 'Terminal app t1, you have control'");
    browser.run(&["press", "q"]);
    at.run(&["--session", "app", "wait", "q"]).success();
    assert_eq!(at.received("app"), "q");
    assert!(
        browser.status().contains(&"Keys go to input-echo · ctrl+p ctrl+q moves focus out".to_string()),
        "{:?}",
        browser.status()
    );
    assert_eq!(browser.eval("document.querySelectorAll('.split-grid [role=application]').length"), "1");
}
