use super::{Browser, Dashboard};
use crate::support::{AgentTerminal, Cue, fixture};

fn cli_line(browser: &Browser) -> String {
    browser.text("[role=dialog] code").trim().to_string()
}

fn title(browser: &Browser) -> String {
    browser.text("[role=dialog] h2").trim().to_string()
}

fn focused(browser: &Browser) -> String {
    let value = browser.eval("document.activeElement.value ?? null");
    serde_json::from_str(&value).expect("the focused field's value")
}

fn refusals(browser: &Browser) -> Vec<String> {
    let found = browser.eval("JSON.stringify([...document.querySelectorAll('[role=dialog] [role=alert]')].map((alert) => alert.textContent))");
    let found: String = serde_json::from_str(&found).expect("a JSON string");
    serde_json::from_str(&found).expect("the refusals")
}

fn focused_label(browser: &Browser) -> String {
    let label = browser.eval("document.activeElement.labels?.[0]?.textContent ?? null");
    serde_json::from_str(&label).expect("the focused field's label")
}

fn submit(browser: &Browser, label: &str) {
    let button = "[role=dialog] button[type=submit]";
    assert_eq!(browser.text(button).trim(), label);
    browser.run(&["click", button]);
}

fn wait_for_cli_line(browser: &Browser, line: &str) {
    browser.wait_until(&format!(
        "document.querySelector('[role=dialog] code')?.textContent === {}",
        serde_json::to_string(line).unwrap()
    ));
}

fn wait_for_dialog(browser: &Browser) {
    browser.wait_until("document.querySelector('[role=dialog]') !== null");
}

#[test]
fn new_session_writes_the_cli_line_as_the_form_fills_then_opens_the_tab_and_goes_live() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    let cwd = at.home().to_string_lossy().into_owned();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/"));
    browser.wait_for_text("No active sessions");

    browser.click_button_in("main", "New session");
    wait_for_dialog(&browser);
    assert_eq!(title(&browser), "New session");
    assert_eq!(focused(&browser), "session-1");
    assert_eq!(cli_line(&browser), "agent-terminal --session session-1 open <program> --size 120x40");

    browser.fill("Session name", "qa");
    assert_eq!(cli_line(&browser), "agent-terminal --session qa open <program> --size 120x40");
    let command = format!("{} --lines 0 --cue {}", fixture("line-printer"), cue.path());
    browser.fill("Command", &command);
    assert_eq!(cli_line(&browser), format!("agent-terminal --session qa open {command} --size 120x40"));
    browser.fill("Working directory", &cwd);
    browser.check("80×24");
    assert_eq!(cli_line(&browser), format!("agent-terminal --session qa open {command} --cwd {cwd} --size 80x24"));
    assert_eq!(browser.run(&["is", "enabled", "[role=dialog] [role=switch]"]).trim(), "false");
    assert!(browser.text("[role=dialog]").contains("Shells only"));
    browser.click_button("Copy the command line");
    browser.wait_until("document.querySelector('.toast') !== null");
    assert_eq!(
        browser.text(".toast").lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>(),
        ["Copied", &format!("agent-terminal --session qa open {command} --cwd {cwd} --size 80x24")]
    );

    submit(&browser, "Create");
    browser.wait_until("location.pathname === '/s/qa/t1'");
    assert_eq!(browser.eval("document.querySelector('[role=dialog]')"), "null");
    browser.wait_for_text("Waiting for first frame...");
    browser.wait_for_status("Connected");
    let enabled =
        |tooltip: &str| browser.run(&["is", "enabled", &format!("button[data-tooltip='{tooltip}']")]).trim() == "true";
    assert!(!enabled("Restart") && !enabled("Screenshot"), "restart and screenshot wait for the first frame");
    cue.print("first\n");
    browser.wait_for_status("Live");
    assert!(enabled("Restart") && enabled("Screenshot"));
    browser.wait_for_screen("first");
    assert!(!browser.text("main").contains("Waiting for first frame..."));
    assert_eq!(at.run(&["session", "list"]).success().stdout(), "qa t1 · line-printer · 80x24 · running · current\n");
    assert_eq!(at.run(&["--session", "qa", "get", "cwd"]).success().stdout(), format!("{cwd}\n"));
}

#[test]
fn new_session_refuses_a_missing_invalid_or_taken_name_a_missing_command_a_size_out_of_range_and_an_unknown_program() {
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
    let browser = Browser::open(&dashboard.url("/s/qa/t1"));
    browser.wait_for_status("Live");

    browser.click_button("New session");
    wait_for_dialog(&browser);
    assert_eq!(focused(&browser), "session-2");
    browser.fill("Session name", " ");
    browser.check("Custom");
    browser.fill("Columns", "10");
    wait_for_cli_line(&browser, "agent-terminal --session <name> open <program> --size 10x40");
    submit(&browser, "Create");
    browser.wait_until("document.querySelectorAll('[role=dialog] [role=alert]').length === 3");
    assert_eq!(refusals(&browser), ["Enter a session name.", "Enter a command.", "Columns 20–500, rows 5–200."]);
    assert_eq!(focused_label(&browser), "Session name");

    browser.fill("Session name", "q a");
    browser.fill("Command", "lazygit");
    browser.fill("Columns", "80");
    wait_for_cli_line(&browser, "agent-terminal --session 'q a' open lazygit --size 80x40");
    browser.run(&["press", "Enter"]);
    browser.wait_until("document.querySelectorAll('[role=dialog] [role=alert]').length === 1");
    assert_eq!(refusals(&browser), ["Use letters, numbers, - and _."]);

    browser.fill("Session name", "qa");
    wait_for_cli_line(&browser, "agent-terminal --session qa open lazygit --size 80x40");
    submit(&browser, "Create");
    browser.wait_for_text("A session named qa already exists.");
    assert_eq!(refusals(&browser), ["A session named qa already exists."]);

    browser.fill("Session name", "fresh");
    browser.fill("Command", "lazyget-46h35 --all");
    wait_for_cli_line(&browser, "agent-terminal --session fresh open lazyget-46h35 --all --size 80x40");
    submit(&browser, "Create");
    browser.wait_for_text("command not found: lazyget-46h35");
    assert_eq!(refusals(&browser), Vec::<String>::new());
    assert_eq!(
        browser.text(".toast").lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>(),
        ["command not found: lazyget-46h35"]
    );
    assert_eq!(title(&browser), "New session");
    assert_eq!(focused_label(&browser), "Command");
    assert_eq!(at.run(&["session", "list"]).success().stdout(), "qa t1 · line-printer · 30x4 · running · current\n");
}

#[test]
fn the_cli_line_quotes_a_session_name_the_shell_would_split() {
    let at = AgentTerminal::new();
    at.run(&[
        "--session",
        "two words",
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
    let browser = Browser::open(&dashboard.url("/"));
    browser.wait_until("location.pathname === '/s/two%20words/t1'");

    browser.click_button("New tab in two words");
    wait_for_dialog(&browser);
    assert_eq!(cli_line(&browser), "agent-terminal --session 'two words' open <program> --size 120x40");
}

#[test]
fn new_tab_opens_another_terminal_in_the_session_it_was_asked_from() {
    let at = AgentTerminal::new();
    for session in ["build", "qa"] {
        at.run(&[
            "--session",
            session,
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
    }
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/build/t1"));
    browser.wait_for_status("Live");

    browser.click_button("New tab in qa");
    wait_for_dialog(&browser);
    assert_eq!(title(&browser), "New tab");
    assert_eq!(focused_label(&browser), "Command");
    assert_eq!(browser.run(&["is", "checked", "[role=dialog] input[value=existing]"]).trim(), "true");
    assert_eq!(browser.run(&["get", "value", "[role=dialog] select"]).trim(), "qa");
    assert_eq!(cli_line(&browser), "agent-terminal --session qa open <program> --size 120x40");

    browser.check("New session");
    assert_eq!(title(&browser), "New session");
    assert_eq!(cli_line(&browser), "agent-terminal --session session-3 open <program> --size 120x40");
    browser.check("Existing session");
    assert_eq!(title(&browser), "New tab");

    let command = format!("{} --lines 2 --hold", fixture("line-printer"));
    browser.fill("Command", &command);
    browser.check("200×60");
    assert_eq!(cli_line(&browser), format!("agent-terminal --session qa open {command} --size 200x60"));
    submit(&browser, "Open tab");
    browser.wait_until("location.pathname === '/s/qa/t2'");
    browser.wait_for_status("Live");
    browser.wait_for_screen("line 2");
    assert_eq!(
        at.run(&["--session", "qa", "tab"]).success().stdout(),
        "qa t1 · line-printer · 30x4 · running\nqa t2 · line-printer · 200x60 · running · current\n"
    );
}

#[test]
fn shell_integration_is_on_for_shells_and_turns_open_into_shell() {
    let at = AgentTerminal::new();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/"));
    browser.wait_for_text("No active sessions");
    browser.click_button("New session");
    wait_for_dialog(&browser);

    browser.fill("Command", "bash");
    assert_eq!(browser.run(&["is", "enabled", "[role=dialog] [role=switch]"]).trim(), "true");
    assert_eq!(browser.run(&["get", "attr", "[role=dialog] [role=switch]", "aria-checked"]).trim(), "true");
    assert!(browser.text("[role=dialog]").contains("Records each command with its exit code"));
    assert_eq!(cli_line(&browser), "agent-terminal --session session-1 shell --shell bash --size 120x40");

    browser.run(&["find", "role", "switch", "click", "--name", "Shell integration", "--exact"]);
    assert_eq!(browser.run(&["get", "attr", "[role=dialog] [role=switch]", "aria-checked"]).trim(), "false");
    assert_eq!(cli_line(&browser), "agent-terminal --session session-1 open bash --size 120x40");
    browser.run(&["find", "role", "switch", "click", "--name", "Shell integration", "--exact"]);
    assert_eq!(cli_line(&browser), "agent-terminal --session session-1 shell --shell bash --size 120x40");

    submit(&browser, "Create");
    browser.wait_until("location.pathname === '/s/session-1/t1'");
    browser.wait_for_status("Live");
    let ran = at.run(&["--session", "session-1", "exec", "echo integrated"]).success();
    assert!(ran.stdout().contains("integrated"), "{}", ran.stdout());
    assert_eq!(browser.text("[role=group][aria-label='Command line'] span").trim(), "bash");
}
