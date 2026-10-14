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

fn newest_toast(browser: &Browser) -> Vec<String> {
    browser.text(".toast").lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect()
}

fn wait_for_button(browser: &Browser, name: &str) {
    browser.wait_until(&format!(
        "[...document.querySelectorAll('button')].some((button) => button.textContent === {})",
        serde_json::to_string(name).unwrap()
    ));
}

fn wait_for_toast(browser: &Browser, title: &str) {
    browser.wait_until(&format!(
        "document.querySelector('.toast .font-medium')?.textContent === {}",
        serde_json::to_string(title).unwrap()
    ));
}

fn record_button(browser: &Browser) -> (String, String, String) {
    let found = browser.eval(
        "JSON.stringify(['Start recording', 'Stop recording'].flatMap((name) => [...document.querySelectorAll('button')].filter((button) => button.textContent === name).map((button) => [name, button.getAttribute('aria-pressed'), button.dataset.tooltip])))",
    );
    let found: String = serde_json::from_str(&found).expect("a JSON string");
    let mut found: Vec<(String, String, String)> = serde_json::from_str(&found).expect("the record button");
    assert_eq!(found.len(), 1, "one record button: {found:?}");
    found.remove(0)
}

#[test]
fn record_saves_the_tab_to_a_cast_file_until_stopped_and_follows_record_from_the_cli() {
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
    let saved = at.home().join("recordings").join(format!("qa-t1-{}.cast", frame.trim()));
    let saved = saved.to_string_lossy().into_owned();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/qa/t1"));
    browser.wait_for_status("Live");
    assert_eq!(record_button(&browser), ("Start recording".into(), "false".into(), "Record".into()));

    browser.click_button("Start recording");
    wait_for_button(&browser, "Stop recording");
    wait_for_toast(&browser, "Recording");
    assert_eq!(newest_toast(&browser), ["Recording", saved.as_str()]);
    assert_eq!(record_button(&browser), ("Stop recording".into(), "true".into(), format!("Recording to {saved}")));

    browser.click_button("Stop recording");
    wait_for_button(&browser, "Start recording");
    wait_for_toast(&browser, "Recording saved");
    assert_eq!(newest_toast(&browser), ["Recording saved", saved.as_str()]);
    assert_eq!(record_button(&browser), ("Start recording".into(), "false".into(), "Record".into()));
    let cast = std::fs::read_to_string(&saved).expect("the saved recording");
    assert!(cast.starts_with("{\"version\":2,"), "{cast}");

    let chosen = at.home().join("chosen.cast").to_string_lossy().into_owned();
    at.run(&["--session", "qa", "record", "start", &chosen]).success();
    wait_for_button(&browser, "Stop recording");
    assert_eq!(record_button(&browser), ("Stop recording".into(), "true".into(), format!("Recording to {chosen}")));
    at.run(&["--session", "qa", "record", "stop"]).success();
    wait_for_button(&browser, "Start recording");

    at.run(&["--session", "qa", "press", "Enter"]).success();
    browser.wait_for_status("Exited");
    browser.wait_until(
        "[...document.querySelectorAll('button')].find((button) => button.textContent === 'Start recording').disabled",
    );
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
    let pid = || at.run(&["--session", "qa", "get", "pid"]).success().stdout();
    let first = pid();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/qa/t1"));
    browser.wait_for_status("Live");

    browser.click_button("Restart line-printer");
    browser.wait_until("document.querySelector('[role=alertdialog]') !== null");
    assert_eq!(
        dialog(&browser),
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
    assert_eq!(pid(), first);

    browser.click_button("Restart line-printer");
    browser.wait_until("document.querySelector('[role=alertdialog]') !== null");
    browser.click_button("Restart");
    browser.wait_until("document.querySelector('[role=alertdialog]') === null");
    at.run(&["--session", "qa", "wait", "line 2"]).success();
    let second = pid();
    assert_ne!(second, first);

    at.run(&["--session", "qa", "press", "Enter"]).success();
    browser.wait_for_status("Exited");
    browser.click_button("Restart line-printer");
    browser.wait_for_status("Live");
    assert_eq!(browser.eval("document.querySelector('[role=alertdialog]')"), "null");
    assert_eq!(at.run(&["--session", "qa", "get", "exit-code"]).success().stdout(), "running\n");
    assert_ne!(pid(), second);
}
