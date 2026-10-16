use super::{Browser, Dashboard, snapshot_rows};
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

fn cell(name: &str) -> String {
    format!("main section[aria-label={name:?}]")
}

#[test]
fn two_up_from_a_tab_shows_it_beside_the_tab_after_it_both_live() {
    let at = AgentTerminal::without_session();
    let cue = open_qa_with_two_tabs_and_web(&at);
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/qa/app"));
    browser.wait_for_status("Live");

    two_up_shows_t1_beside_t2_with_t1_focused(&browser);
    both_cells_show_their_screens(&at, &browser);
    cue.print("streamed\n");
    browser.wait_for_screen_in(&cell("qa api line-printer"), "streamed");
    assert_eq!(browser.screen_in(&cell("qa api line-printer")), ["line 1", "line 2", "streamed"]);

    let (_, frame) = snapshot_rows(&at, &["--session", "qa", "--tab", "app"]);
    browser.wait_for_status(&format!("frame {frame}"));
    assert_eq!(browser.status(), ["Live", &dashboard.stream("qa/app"), "30 × 4", &format!("frame {frame}")]);
}

fn open_qa_with_two_tabs_and_web(at: &AgentTerminal) -> Cue {
    at.run(&["session", "new", "qa"]).success();
    hold(at, &["--session", "qa", "open"], "30x4", "1");
    let cue = Cue::new();
    at.run(&[
        "--session",
        "qa",
        "tab",
        "new",
        "--name",
        "api",
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
    at.run(&["session", "new", "web"]).success();
    hold(at, &["--session", "web", "open"], "30x4", "3");
    cue
}

fn two_up_shows_t1_beside_t2_with_t1_focused(browser: &Browser) {
    browser.click_button("Two terminals side by side");
    browser.wait_until("location.pathname === '/split/qa:app,qa:api'");
    assert_eq!(browser.run(&["get", "title"]).trim(), "Split · 2 terminals · agent-terminal");
    assert_eq!(
        grid(browser),
        [
            "region \"qa app line-printer\"",
            "button \"qa app line-printer , running\"",
            "link \"Maximize qa app\"",
            "button \"Remove qa app from the grid\"",
            "region \"qa api line-printer\"",
            "button \"qa api line-printer , running\"",
            "link \"Maximize qa api\"",
            "button \"Remove qa api from the grid\"",
        ]
    );
    assert_eq!(layout(browser), "\"Two terminals side by side\"");
    assert_eq!(keyboard_focus(browser), "\"Two terminals side by side\"");
    assert_eq!(focused(browser), "\"qa app line-printer\"");
}

fn both_cells_show_their_screens(at: &AgentTerminal, browser: &Browser) {
    browser.wait_for_screen_in(&cell("qa app line-printer"), "line 1");
    browser.wait_for_screen_in(&cell("qa api line-printer"), "line 2");
    assert_eq!(
        browser.screen_in(&cell("qa app line-printer")),
        snapshot_rows(at, &["--session", "qa", "--tab", "app"]).0
    );
    assert_eq!(
        browser.screen_in(&cell("qa api line-printer")),
        snapshot_rows(at, &["--session", "qa", "--tab", "api"]).0
    );
}

#[test]
fn four_up_fills_with_the_current_tab_then_the_ones_after_it_and_single_returns_to_the_focused_tab() {
    let at = AgentTerminal::without_session();
    at.run(&["session", "new", "build"]).success();
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
    at.run(&["session", "new", "chat"]).success();
    hold(&at, &["--session", "chat", "open"], "30x4", "2");
    at.run(&["session", "new", "qa"]).success();
    hold(&at, &["--session", "qa", "open"], "30x4", "1");
    hold(&at, &["--session", "qa", "tab", "new"], "30x4", "3");
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/chat/app"));
    browser.wait_for_status("Live");

    four_up_fills_from_the_current_tab(&browser);
    the_cells_show_their_screens_and_the_exit(&browser);
    browser.click_button("Two terminals side by side");
    browser.wait_until("location.pathname === '/split/chat:app,qa:app'");
    assert_eq!(focused(&browser), "\"chat app line-printer\"");
    single_returns_to_the_focused_tab(&browser);
}

fn four_up_fills_from_the_current_tab(browser: &Browser) {
    browser.click_button("Four terminals");
    browser.wait_until("location.pathname === '/split/chat:app,qa:app,qa:api,build:app'");
    assert_eq!(browser.run(&["get", "title"]).trim(), "Split · 4 terminals · agent-terminal");
    assert_eq!(layout(browser), "\"Four terminals\"");
    assert_eq!(focused(browser), "\"chat app line-printer\"");
    browser.wait_for_text("exited with code");
    assert_eq!(
        grid(browser),
        [
            "region \"chat app line-printer\"",
            "button \"chat app line-printer , running\"",
            "link \"Maximize chat app\"",
            "button \"Remove chat app from the grid\"",
            "region \"qa app line-printer\"",
            "button \"qa app line-printer , running\"",
            "link \"Maximize qa app\"",
            "button \"Remove qa app from the grid\"",
            "region \"qa api line-printer\"",
            "button \"qa api line-printer , running\"",
            "link \"Maximize qa api\"",
            "button \"Remove qa api from the grid\"",
            "region \"build app line-printer\"",
            "button \"build app line-printer exited 3 , exited 3\"",
            "link \"Maximize build app\"",
            "button \"Remove build app from the grid\"",
        ]
    );
}

fn the_cells_show_their_screens_and_the_exit(browser: &Browser) {
    assert_eq!(
        browser.text(&format!("{} [role=status]", cell("build app line-printer"))).trim(),
        "line-printer exited with code 3"
    );
    browser.wait_for_screen_in(&cell("qa api line-printer"), "line 3");
    assert_eq!(browser.screen_in(&cell("qa api line-printer")), ["line 1", "line 2", "line 3"]);
    browser.wait_for_screen_in(&cell("build app line-printer"), "line 1");
    assert_eq!(browser.screen_in(&cell("build app line-printer")), ["line 1"]);
}

fn single_returns_to_the_focused_tab(browser: &Browser) {
    browser.click_button("Single terminal");
    browser.wait_until("location.pathname === '/s/chat/app'");
    browser.wait_for_status("Live");
    assert_eq!(layout(browser), "\"Single terminal\"");
    assert_eq!(keyboard_focus(browser), "\"Single terminal\"");
}

fn open_qa_with_two_tabs_and_web_of_three_sizes(at: &AgentTerminal) {
    at.run(&["session", "new", "qa"]).success();
    hold(at, &["--session", "qa", "open"], "30x4", "1");
    hold(at, &["--session", "qa", "tab", "new"], "32x5", "2");
    at.run(&["session", "new", "web"]).success();
    hold(at, &["--session", "web", "open"], "34x6", "3");
}

#[test]
fn removing_a_cell_offers_the_picker_and_picking_a_tab_fills_and_focuses_that_cell() {
    let at = AgentTerminal::without_session();
    open_qa_with_two_tabs_and_web_of_three_sizes(&at);
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/split/qa:app,qa:api"));
    browser.wait_for_status("Live");

    removing_a_cell_offers_the_picker(&browser);
    picking_a_tab_fills_and_focuses_the_cell(&at, &browser, &dashboard);
    browser.click_button("Remove web app from the grid");
    browser.wait_until("location.pathname === '/split/qa:app,_'");
    assert_eq!(focused(&browser), "\"qa app line-printer\"");
    browser.wait_for_status("30 × 4");
}

fn removing_a_cell_offers_the_picker(browser: &Browser) {
    browser.click_button("Remove qa api from the grid");
    browser.wait_until("location.pathname === '/split/qa:app,_'");
    assert_eq!(
        grid(browser),
        [
            "region \"qa app line-printer\"",
            "button \"qa app line-printer , running\"",
            "link \"Maximize qa app\"",
            "button \"Remove qa app from the grid\"",
            "region \"Show a terminal\"",
            "heading \"Show a terminal\"",
            "button \"app line-printer shown , qa app\"",
            "button \"api line-printer , qa api\"",
            "button \"app line-printer , web app\"",
        ]
    );
    assert_eq!(
        browser.eval(
            "[...document.querySelectorAll('.split-grid section[aria-labelledby] button')].map((button) => button.disabled).join()"
        ),
        "\"true,false,false\""
    );
    assert_eq!(focused(browser), "\"qa app line-printer\"");
    assert_eq!(browser.eval("document.activeElement.id"), "\"main-content\"");
}

fn picking_a_tab_fills_and_focuses_the_cell(at: &AgentTerminal, browser: &Browser, dashboard: &Dashboard) {
    browser.click_button("app line-printer , web app");
    browser.wait_until("location.pathname === '/split/qa:app,web:app'");
    assert_eq!(focused(browser), "\"web app line-printer\"");
    assert_eq!(
        browser.eval("document.activeElement.closest('section')?.ariaLabel ?? 'none'"),
        "\"web app line-printer\""
    );
    let (_, frame) = snapshot_rows(at, &["--session", "web"]);
    browser.wait_for_status(&format!("frame {frame}"));
    browser.wait_for_screen_in(&cell("web app line-printer"), "line 3");
    assert_eq!(browser.status(), ["Live", &dashboard.stream("web/app"), "34 × 6", &format!("frame {frame}")]);
}

#[test]
fn clicking_a_cell_focuses_it_and_a_tab_picked_in_the_sidebar_replaces_the_focused_cell() {
    let at = AgentTerminal::without_session();
    open_qa_with_two_tabs_and_web_of_three_sizes(&at);
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/split/qa:app,qa:api"));
    browser.wait_for_status("30 × 4");
    assert_eq!(selected_in_sidebar(&browser), "\"line-printer, qa app, running\"");

    clicking_a_cell_focuses_it_and_selects_it_in_the_sidebar(&at, &browser, &dashboard);
    a_tab_picked_in_the_sidebar_replaces_the_focused_cell(&at, &browser);
}

fn clicking_a_cell_focuses_it_and_selects_it_in_the_sidebar(
    at: &AgentTerminal,
    browser: &Browser,
    dashboard: &Dashboard,
) {
    browser.wait_for_screen_in(&cell("qa api line-printer"), "line 2");
    browser.run(&["click", &format!("{} [role=group]", cell("qa api line-printer"))]);
    let (_, frame) = snapshot_rows(at, &["--session", "qa", "--tab", "api"]);
    browser.wait_for_status("32 × 5");
    browser.wait_for_status(&format!("frame {frame}"));
    assert_eq!(focused(browser), "\"qa api line-printer\"");
    assert_eq!(browser.status(), ["Live", &dashboard.stream("qa/api"), "32 × 5", &format!("frame {frame}")]);
    assert_eq!(selected_in_sidebar(browser), "\"line-printer, qa api, running\"");
    assert_eq!(browser.eval("location.pathname"), "\"/split/qa:app,qa:api\"");
}

fn a_tab_picked_in_the_sidebar_replaces_the_focused_cell(at: &AgentTerminal, browser: &Browser) {
    browser.run(&["find", "role", "link", "click", "--name", "app line-printer , web app, running", "--exact"]);
    browser.wait_until("location.pathname === '/split/qa:app,web:app'");
    assert_eq!(focused(browser), "\"web app line-printer\"");
    browser.wait_for_status("34 × 6");
    browser.wait_for_screen_in(&cell("web app line-printer"), "line 3");
    assert_eq!(browser.screen_in(&cell("web app line-printer")), snapshot_rows(at, &["--session", "web"]).0);
}

#[test]
fn maximize_shows_the_tab_alone_and_a_split_route_survives_a_reload() {
    let at = AgentTerminal::without_session();
    at.run(&["session", "new", "qa"]).success();
    hold(&at, &["--session", "qa", "open"], "30x4", "1");
    at.run(&["session", "new", "web-app"]).success();
    hold(&at, &["--session", "web-app", "open"], "32x5", "2");
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/qa/app"));
    browser.wait_for_status("Live");

    browser.click_button("Two terminals side by side");
    browser.wait_until("location.pathname === '/split/qa:app,web-app:app'");
    browser.run(&["reload"]);
    browser.wait_for_screen_in(&cell("web-app app line-printer"), "line 2");
    assert_eq!(focused(&browser), "\"qa app line-printer\"");

    browser.run(&["find", "role", "link", "click", "--name", "Maximize web-app app", "--exact"]);
    browser.wait_until("location.pathname === '/s/web-app/app'");
    assert_eq!(browser.eval("document.activeElement.id"), "\"main-content\"");
    browser.wait_for_status("32 × 5");
    browser.wait_for_screen("line 2");
    assert_eq!(layout(&browser), "\"Single terminal\"");
    assert_eq!(browser.run(&["get", "title"]).trim(), "web-app app · line-printer · agent-terminal");
}

#[test]
fn a_split_route_with_three_cells_is_not_a_page() {
    let at = AgentTerminal::without_session();
    at.run(&["session", "new", "qa"]).success();
    hold(&at, &["--session", "qa", "open"], "30x4", "1");
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/split/qa:app,_,_"));

    browser.wait_for_text("This page could not be found.");
    assert_eq!(browser.run(&["get", "title"]).trim(), "404: This page could not be found · agent-terminal");
}

#[test]
fn the_focused_cell_of_a_split_takes_the_keys_of_a_human_in_control() {
    let at = AgentTerminal::without_session();
    at.run(&["session", "new", "app"]).success();
    at.run(&["--session", "app", "open", "--name", "app", "--size", "40x6", "--wait", "ready", &fixture("input-echo")])
        .success();
    at.run(&["session", "new", "web"]).success();
    hold(&at, &["--session", "web", "open"], "30x4", "1");
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/split/app:app,web:app"));
    browser.wait_for_status("Live");
    browser.wait_for_screen_in(&cell("app app input-echo"), "ready");

    browser.click_button("Take control");
    browser.wait_until("document.activeElement?.getAttribute('aria-label') === 'Terminal app app, you have control'");
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
