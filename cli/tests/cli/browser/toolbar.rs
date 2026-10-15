use super::{Browser, Dashboard};
use crate::support::{AgentTerminal, fixture};

fn command_line(browser: &Browser) -> Vec<(String, String)> {
    let parts = browser.eval(
        "JSON.stringify([...document.querySelector('[role=group][aria-label=\"Command line\"]').children].map((part) => [part.textContent, getComputedStyle(part).color]))",
    );
    let parts: String = serde_json::from_str(&parts).expect("a JSON string");
    serde_json::from_str(&parts).expect("the command line parts")
}

#[test]
fn the_toolbar_shows_the_command_line_with_its_working_directory_dimmed() {
    let at = AgentTerminal::new();
    let cwd = at.home().to_string_lossy().into_owned();
    at.run(&[
        "--session",
        "qa",
        "open",
        "--size",
        "30x4",
        "--cwd",
        &cwd,
        "--wait",
        "two words",
        "sh",
        "-c",
        "echo two words; read line",
    ])
    .success();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/qa/t1"));
    browser.wait_for_status("Live");

    let parts = command_line(&browser);
    let texts: Vec<&str> = parts.iter().map(|(text, _)| text.as_str()).collect();
    assert_eq!(texts, ["sh -c 'echo two words; read line'", cwd.as_str()]);
    assert_ne!(parts[0].1, parts[1].1, "the working directory is dimmed");
}

fn toolbar_buttons(browser: &Browser) -> Vec<String> {
    browser.eval_json(
        "[...document.querySelector('[role=group][aria-label=\"Command line\"]').nextElementSibling.querySelectorAll('button')].map((button) => button.textContent)",
    )
}

#[test]
fn the_toolbar_offers_restart_screenshot_layouts_and_control() {
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

    assert_eq!(
        toolbar_buttons(&browser),
        [
            "Restart line-printer",
            "Take a screenshot",
            "Single terminal",
            "Two terminals side by side",
            "Four terminals",
            "Take control",
        ]
    );
}

fn newest_toast(browser: &Browser) -> Vec<String> {
    browser.text(".toast").lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect()
}

fn wait_for_toast(browser: &Browser, title: &str) {
    browser.wait_until(&format!(
        "document.querySelector('.toast .font-medium')?.textContent === {}",
        serde_json::to_string(title).unwrap()
    ));
}

#[test]
fn screenshot_saves_a_png_of_the_screen_and_says_where() {
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
    let frame = at.run(&["--session", "qa", "get", "frame"]).success().stdout();
    let file = format!("qa-t1-{}.png", frame.trim());
    let saved = at.home().join("screenshots").join(&file).to_string_lossy().into_owned();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/qa/t1"));
    browser.wait_for_status("Live");

    browser.click_button("Take a screenshot");
    wait_for_toast(&browser, &format!("Saved {file}"));
    assert_eq!(newest_toast(&browser), [format!("Saved {file}"), saved.clone()]);
    let png = std::fs::read(&saved).expect("the screenshot");
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG: {:?}", &png[..8.min(png.len())]);
}

fn dialog(browser: &Browser) -> Vec<String> {
    browser
        .text("[role=alertdialog]")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn qa_pid(at: &AgentTerminal) -> String {
    at.run(&["--session", "qa", "get", "pid"]).success().stdout()
}

#[test]
fn restart_asks_first_while_the_program_runs_and_restarts_at_once_once_it_exited() {
    let at = AgentTerminal::new();
    at.run(&[
        "--session",
        "qa",
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
    let first = qa_pid(&at);
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/qa/t1"));
    browser.wait_for_status("Live");

    cancelling_the_question_keeps_the_program(&at, &browser, &first);
    let second = confirming_restarts_the_running_program(&at, &browser, &first);
    an_exited_program_restarts_without_asking(&at, &browser, &second);
}

fn cancelling_the_question_keeps_the_program(at: &AgentTerminal, browser: &Browser, first: &str) {
    browser.click_button("Restart line-printer");
    browser.wait_until("document.querySelector('[role=alertdialog]') !== null");
    assert_eq!(
        dialog(browser),
        [
            "Restart line-printer?",
            "qa t1 is killed and started again with the same command, size and working directory.",
            "Cancel",
            "Restart",
        ]
    );
    assert_eq!(browser.eval("document.activeElement.textContent"), "\"Cancel\"");
    browser.click_button("Cancel");
    browser.wait_until("document.querySelector('[role=alertdialog]') === null");
    assert_eq!(browser.eval("document.activeElement.textContent"), "\"Restart line-printer\"");
    assert_eq!(qa_pid(at), first);
}

fn confirming_restarts_the_running_program(at: &AgentTerminal, browser: &Browser, first: &str) -> String {
    browser.click_button("Restart line-printer");
    browser.wait_until("document.querySelector('[role=alertdialog]') !== null");
    browser.click_button("Restart");
    browser.wait_until("document.querySelector('[role=alertdialog]') === null");
    at.run(&["--session", "qa", "wait", "line 2"]).success();
    let second = qa_pid(at);
    assert_ne!(second, first);
    second
}

fn an_exited_program_restarts_without_asking(at: &AgentTerminal, browser: &Browser, second: &str) {
    at.run(&["--session", "qa", "press", "Enter"]).success();
    browser.wait_for_status("Exited");
    browser.click_button("Restart line-printer");
    browser.wait_for_status("Live");
    assert_eq!(browser.eval("document.querySelector('[role=alertdialog]')"), "null");
    assert_eq!(at.run(&["--session", "qa", "get", "exit-code"]).success().stdout(), "running\n");
    assert_ne!(qa_pid(at), second);
}
