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

fn enabled(browser: &Browser, tooltip: &str) -> bool {
    browser.run(&["is", "enabled", &format!("button[data-tooltip='{tooltip}']")]).trim() == "true"
}

fn open_line_printer(at: &AgentTerminal, session: &str) {
    at.run(&["session", "new", session]).success();
    at.run(&[
        "--session",
        session,
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
}

#[test]
fn new_session_writes_the_cli_line_as_the_form_fills_then_opens_the_tab_and_goes_live() {
    let at = AgentTerminal::without_session();
    let cue = Cue::new();
    let cwd = at.home().to_string_lossy().into_owned();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/"));
    browser.wait_for_text("No active sessions");

    the_dialog_opens_with_a_free_session_name(&browser);
    let command = format!("{} --lines 0 --cue {}", fixture("line-printer"), cue.path());
    the_cli_line_follows_the_form(&browser, &command, &cwd);
    copying_shows_the_cli_line_in_a_toast(&browser, &command, &cwd);
    creating_opens_the_tab_waiting_for_its_first_frame(&browser);
    the_first_frame_makes_the_tab_live(&at, &browser, &cue, &cwd);
}

fn the_dialog_opens_with_a_free_session_name(browser: &Browser) {
    browser.click_button_in("main", "New session");
    wait_for_dialog(browser);
    assert_eq!(title(browser), "New session");
    assert_eq!(focused(browser), "session-1");
    assert_eq!(
        cli_line(browser),
        "agent-terminal session new session-1 && agent-terminal --session session-1 open --name <tab> <program> --size 120x40"
    );
}

fn the_cli_line_follows_the_form(browser: &Browser, command: &str, cwd: &str) {
    browser.fill("Session name", "qa");
    assert_eq!(
        cli_line(browser),
        "agent-terminal session new qa && agent-terminal --session qa open --name <tab> <program> --size 120x40"
    );
    browser.fill("Command", command);
    assert_eq!(
        cli_line(browser),
        format!(
            "agent-terminal session new qa && agent-terminal --session qa open --name line-printer {command} --size 120x40"
        )
    );
    browser.fill("Working directory", cwd);
    browser.check("80×24");
    assert_eq!(
        cli_line(browser),
        format!(
            "agent-terminal session new qa && agent-terminal --session qa open --name line-printer {command} --cwd {cwd} --size 80x24"
        )
    );
    assert_eq!(browser.run(&["is", "enabled", "[role=dialog] [role=switch]"]).trim(), "false");
    assert!(browser.text("[role=dialog]").contains("Shells only"));
}

fn copying_shows_the_cli_line_in_a_toast(browser: &Browser, command: &str, cwd: &str) {
    browser.click_button("Copy the command line");
    browser.wait_until("document.querySelector('.toast') !== null");
    assert_eq!(
        browser.text(".toast").lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>(),
        [
            "Copied",
            &format!(
                "agent-terminal session new qa && agent-terminal --session qa open --name line-printer {command} --cwd {cwd} --size 80x24"
            )
        ]
    );
}

fn creating_opens_the_tab_waiting_for_its_first_frame(browser: &Browser) {
    submit(browser, "Create");
    browser.wait_until("location.pathname === '/s/qa/line-printer'");
    assert_eq!(browser.eval("document.querySelector('[role=dialog]')"), "null");
    browser.wait_for_text("Waiting for first frame...");
    browser.wait_for_status("Connected");
    assert!(
        !enabled(browser, "Restart") && !enabled(browser, "Screenshot"),
        "restart and screenshot wait for the first frame"
    );
    assert_ne!(
        browser.eval("document.querySelector('.sr-only[aria-live=polite]').textContent"),
        "\"qa line-printer is live\""
    );
}

fn the_first_frame_makes_the_tab_live(at: &AgentTerminal, browser: &Browser, cue: &Cue, cwd: &str) {
    cue.print("first\n");
    browser.wait_for_status("Live");
    browser.wait_for_announcement("qa line-printer is live");
    assert!(enabled(browser, "Restart") && enabled(browser, "Screenshot"));
    browser.wait_for_screen("first");
    assert!(!browser.text("main").contains("Waiting for first frame..."));
    assert_eq!(
        at.run(&["session", "list"]).success().stdout(),
        "qa line-printer · line-printer · 80x24 · running · current\n"
    );
    assert_eq!(at.run(&["--session", "qa", "get", "cwd"]).success().stdout(), format!("{cwd}\n"));
}

#[test]
fn new_session_refuses_a_missing_invalid_or_taken_name_a_missing_command_a_size_out_of_range_and_an_unknown_program() {
    let at = AgentTerminal::without_session();
    open_line_printer(&at, "qa");
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/qa/app"));
    browser.wait_for_status("Live");

    empty_fields_and_a_size_out_of_range_are_refused(&browser);
    a_name_outside_the_rule_is_refused(&browser);
    a_taken_name_is_refused(&browser);
    an_unknown_program_is_refused_and_the_form_stays_open(&at, &browser);
}

fn empty_fields_and_a_size_out_of_range_are_refused(browser: &Browser) {
    browser.click_button("New session");
    wait_for_dialog(browser);
    assert_eq!(focused(browser), "session-2");
    browser.fill("Session name", " ");
    browser.check("Custom");
    browser.fill("Columns", "10");
    wait_for_cli_line(
        browser,
        "agent-terminal session new <name> && agent-terminal --session <name> open --name <tab> <program> --size 10x40",
    );
    submit(browser, "Create");
    browser.wait_until("document.querySelectorAll('[role=dialog] [role=alert]').length === 4");
    assert_eq!(
        refusals(browser),
        [
            "Enter a session name.",
            "Enter a command.",
            "Name the tab after the program or app it runs, e.g. api.",
            "Columns 20–500, rows 5–200."
        ]
    );
    assert_eq!(focused_label(browser), "Session name");
}

fn a_name_outside_the_rule_is_refused(browser: &Browser) {
    browser.fill("Session name", "Q-A");
    browser.fill("Command", "lazygit");
    browser.fill("Columns", "80");
    wait_for_cli_line(
        browser,
        "agent-terminal session new Q-A && agent-terminal --session Q-A open --name lazygit lazygit --size 80x40",
    );
    browser.run(&["press", "Enter"]);
    browser.wait_until("document.querySelectorAll('[role=dialog] [role=alert]').length === 1");
    assert_eq!(refusals(browser), ["Use up to 24 of a-z, 0-9 and -, starting with a letter or digit."]);
}

fn a_taken_name_is_refused(browser: &Browser) {
    browser.fill("Session name", "qa");
    wait_for_cli_line(
        browser,
        "agent-terminal session new qa && agent-terminal --session qa open --name lazygit lazygit --size 80x40",
    );
    submit(browser, "Create");
    browser.wait_for_text("A session named qa already exists.");
    assert_eq!(refusals(browser), ["A session named qa already exists."]);
}

fn an_unknown_program_is_refused_and_the_form_stays_open(at: &AgentTerminal, browser: &Browser) {
    browser.fill("Session name", "fresh");
    browser.fill("Command", "lazyget-46h35 --all");
    wait_for_cli_line(
        browser,
        "agent-terminal session new fresh && agent-terminal --session fresh open --name lazyget-46h35 lazyget-46h35 --all --size 80x40",
    );
    submit(browser, "Create");
    browser.wait_for_text("command not found: lazyget-46h35");
    assert_eq!(refusals(browser), Vec::<String>::new());
    assert_eq!(
        browser.text(".toast").lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>(),
        ["command not found: lazyget-46h35"]
    );
    assert_eq!(title(browser), "New session");
    assert_eq!(focused_label(browser), "Command");
    assert_eq!(at.run(&["session", "list"]).success().stdout(), "qa app · line-printer · 30x4 · running · current\n");
}

#[test]
fn new_tab_opens_another_terminal_in_the_session_it_was_asked_from() {
    let at = AgentTerminal::without_session();
    for session in ["build", "qa"] {
        open_line_printer(&at, session);
    }
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/build/app"));
    browser.wait_for_status("Live");

    the_dialog_asked_from_qa_opens_a_tab_in_qa(&browser);
    choosing_a_new_session_and_back_changes_the_title(&browser);
    opening_the_tab_adds_it_to_qa(&at, &browser);
}

fn the_dialog_asked_from_qa_opens_a_tab_in_qa(browser: &Browser) {
    browser.click_button("New tab in qa");
    wait_for_dialog(browser);
    assert_eq!(title(browser), "New tab");
    assert_eq!(focused_label(browser), "Command");
    assert_eq!(browser.run(&["is", "checked", "[role=dialog] input[value=existing]"]).trim(), "true");
    assert_eq!(browser.run(&["get", "value", "[role=dialog] select"]).trim(), "qa");
    assert_eq!(cli_line(browser), "agent-terminal --session qa open --name <tab> <program> --size 120x40");
}

fn choosing_a_new_session_and_back_changes_the_title(browser: &Browser) {
    browser.check("New session");
    assert_eq!(title(browser), "New session");
    assert_eq!(
        cli_line(browser),
        "agent-terminal session new session-3 && agent-terminal --session session-3 open --name <tab> <program> --size 120x40"
    );
    browser.check("Existing session");
    assert_eq!(title(browser), "New tab");
}

fn opening_the_tab_adds_it_to_qa(at: &AgentTerminal, browser: &Browser) {
    let command = format!("{} --lines 2 --hold", fixture("line-printer"));
    browser.fill("Command", &command);
    browser.check("200×60");
    assert_eq!(
        cli_line(browser),
        format!("agent-terminal --session qa open --name line-printer {command} --size 200x60")
    );
    submit(browser, "Open tab");
    browser.wait_until("location.pathname === '/s/qa/line-printer'");
    browser.wait_for_status("Live");
    browser.wait_for_screen("line 2");
    assert_eq!(
        at.run(&["--session", "qa", "tab"]).success().stdout(),
        "qa app · line-printer · 30x4 · running\nqa line-printer · line-printer · 200x60 · running · current\n"
    );
}

#[test]
fn shell_integration_is_on_for_shells_and_turns_open_into_shell() {
    let at = AgentTerminal::without_session();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/"));
    browser.wait_for_text("No active sessions");
    browser.click_button("New session");
    wait_for_dialog(&browser);

    a_shell_command_turns_shell_integration_on(&browser);
    turning_shell_integration_off_opens_the_shell_as_a_program(&browser);
    creating_runs_the_shell_with_integration(&at, &browser);
}

fn a_shell_command_turns_shell_integration_on(browser: &Browser) {
    browser.fill("Command", "bash");
    assert_eq!(browser.run(&["is", "enabled", "[role=dialog] [role=switch]"]).trim(), "true");
    assert_eq!(browser.run(&["get", "attr", "[role=dialog] [role=switch]", "aria-checked"]).trim(), "true");
    assert!(browser.text("[role=dialog]").contains("Records each command with its exit code"));
    assert_eq!(
        cli_line(browser),
        "agent-terminal session new session-1 && agent-terminal --session session-1 shell --name bash --shell bash --size 120x40"
    );
}

fn turning_shell_integration_off_opens_the_shell_as_a_program(browser: &Browser) {
    browser.run(&["find", "role", "switch", "click", "--name", "Shell integration", "--exact"]);
    assert_eq!(browser.run(&["get", "attr", "[role=dialog] [role=switch]", "aria-checked"]).trim(), "false");
    assert_eq!(
        cli_line(browser),
        "agent-terminal session new session-1 && agent-terminal --session session-1 open --name bash bash --size 120x40"
    );
    browser.run(&["find", "role", "switch", "click", "--name", "Shell integration", "--exact"]);
    assert_eq!(
        cli_line(browser),
        "agent-terminal session new session-1 && agent-terminal --session session-1 shell --name bash --shell bash --size 120x40"
    );
}

fn creating_runs_the_shell_with_integration(at: &AgentTerminal, browser: &Browser) {
    submit(browser, "Create");
    browser.wait_until("location.pathname === '/s/session-1/bash'");
    browser.wait_for_status("Live");
    let ran = at.run(&["--session", "session-1", "exec", "echo integrated"]).success();
    assert!(ran.stdout().contains("integrated"), "{}", ran.stdout());
    assert_eq!(browser.text("[role=group][aria-label='Command line'] span").trim(), "bash");
}
