use serde_json::Value;

use super::{Browser, Dashboard};
use crate::support::{AgentTerminal, fixture};

const CALLS: &str = "[aria-label='Calls to work app'] > li:not([aria-busy])";

fn calls(browser: &Browser) -> Vec<Value> {
    let rows = browser.eval(&format!(
        "[...document.querySelectorAll({})].map((row) => {{ \
            const time = row.querySelector('time'); \
            return {{ time: time.textContent, at: time.dateTime, \
                parts: row.innerText.split('\\n').map((part) => part.trim()).filter(Boolean).slice(1) }}; \
        }})",
        serde_json::to_string(CALLS).unwrap()
    ));
    serde_json::from_str(&rows).expect("the Activity rows")
}

fn wait_for_calls(browser: &Browser, count: usize) {
    browser.wait_until(&format!(
        "document.querySelectorAll({}).length === {count}",
        serde_json::to_string(CALLS).unwrap()
    ));
}

fn duration(ms: u64) -> String {
    let seconds = f64::from(u32::try_from(ms).expect("a call shorter than 49 days")) / 1000.0;
    if ms < 1000 { format!("{ms} ms") } else { format!("{seconds:.1} s") }
}

fn seconds_and_millis(time: &str) -> &str {
    &time[time.len() - 6..]
}

#[test]
fn activity_lists_every_call_to_the_tab_live_with_its_time_duration_and_result() {
    let at = AgentTerminal::new();
    let printer = fixture("line-printer");
    at.run(&["open", "--name", "app", "--size", "40x6", "--wait", "line 3", &printer, "--hold"]).success();
    at.run(&["tab", "new", "--name", "api", "--size", "40x6", "--wait", "line 3", &printer, "--hold"]).success();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/work/app"));
    wait_for_calls(&browser, 1);

    make_a_call_of_every_result_to_t1(&at);
    wait_for_calls(&browser, 8);

    let log: Vec<Value> = at.activity("work").into_iter().filter(|entry| entry["tab"] == "app").collect();
    each_row_shows_its_call(&browser, &log);
    assert_eq!(log[1]["command"], "--tab app snapshot");
    assert_eq!(browser.eval("document.querySelector('#ppanel-activity [data-slot=badge]').textContent"), "\"8\"");
    expanding_a_call_shows_what_it_printed(&browser, &log);
}

fn make_a_call_of_every_result_to_t1(at: &AgentTerminal) {
    at.run(&["--tab", "app", "snapshot"]).success();
    assert_eq!(at.run(&["--tab", "app", "--timeout", "300", "wait", "never shown"]).code(), 3);
    assert_eq!(at.run(&["--tab", "app", "expect", "not there"]).code(), 2);
    assert_eq!(at.run(&["--tab", "app", "get", "cell", "99", "1"]).code(), 1);
    at.run(&["--tab", "app", "press", "Enter"]).success();
    at.run(&["--tab", "app", "wait", "--exit"]).success();
    assert_eq!(at.run(&["--tab", "app", "press", "y"]).code(), 5);
}

fn each_row_shows_its_call(browser: &Browser, log: &[Value]) {
    let shown = calls(browser);
    let results = [
        None,
        None,
        Some("timeout"),
        Some("exit 2 expect failed"),
        Some("exit 1"),
        None,
        None,
        Some("exit 5 process exited"),
    ];
    assert_eq!(shown.len(), results.len());
    for ((row, entry), result) in shown.iter().zip(log).zip(results) {
        the_row_shows_the_call(row, entry, result);
    }
}

fn the_row_shows_the_call(row: &Value, entry: &Value, result: Option<&str>) {
    let time = entry["time"].as_str().unwrap();
    assert_eq!(row["at"], time);
    let text = row["time"].as_str().unwrap();
    assert_eq!(text.len(), "14:02:07.312".len(), "{row}");
    assert_eq!(seconds_and_millis(text), &time[17..23], "{row}");
    let mut parts =
        vec![format!("{} {}", entry["command"].as_str().unwrap(), duration(entry["duration_ms"].as_u64().unwrap()))];
    parts.extend(result.map(str::to_string));
    parts.push("+".into());
    assert_eq!(row["parts"], serde_json::json!(parts), "{entry}");
}

fn expanding_a_call_shows_what_it_printed(browser: &Browser, log: &[Value]) {
    browser.run(&["click", &format!("{CALLS}:nth-child(3) button")]);
    browser.wait_until("document.querySelector('#ppanel-activity pre') !== null");
    assert_eq!(
        browser.eval("document.querySelector('#ppanel-activity pre').textContent"),
        serde_json::to_string(&log[2]["output"]).unwrap()
    );
    assert_eq!(
        browser.eval(&format!("document.querySelector(\"{CALLS}:nth-child(3) button\").ariaExpanded")),
        "\"true\""
    );
}

const LIST: &str = "document.querySelector(\"[aria-label='Calls to work app']\")";
const FOLLOW: &str = "document.querySelector('#ppanel-activity button[aria-pressed]')";
const JUMP_SHOWN: &str =
    "[...document.querySelectorAll('button')].some((b) => b.textContent === 'Jump to latest' && !b.hidden)";

fn at_bottom() -> String {
    format!("(() => {{ const list = {LIST}; return list.scrollHeight - list.scrollTop - list.clientHeight < 2; }})()")
}

#[test]
fn activity_follows_new_calls_until_scrolled_up_then_offers_jump_to_latest_and_clears() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "40x6", "--wait", "line 3", &fixture("line-printer"), "--hold"])
        .success();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/work/app"));
    wait_for_calls(&browser, 1);
    let mut batch = vec!["batch"];
    batch.extend(std::iter::repeat_n("get frame", 60));
    at.run(&batch).success();
    wait_for_calls(&browser, 61);

    the_list_follows_the_latest_call(&browser);
    scrolling_up_stops_following_and_offers_jump_to_latest(&at, &browser);
    jump_to_latest_follows_again(&browser);
    follow_toggles_and_keeps_new_calls_in_view(&at, &browser);
    clearing_empties_the_list_until_the_next_call(&at, &browser);
    reopening_the_tab_starts_a_new_list(&at, &browser);
}

fn the_list_follows_the_latest_call(browser: &Browser) {
    browser.wait_until(&at_bottom());
    assert_eq!(browser.eval(&format!("{FOLLOW}.textContent + ' ' + {FOLLOW}.ariaPressed")), "\"Follow true\"");
    assert_eq!(browser.eval(JUMP_SHOWN), "false");
}

fn scrolling_up_stops_following_and_offers_jump_to_latest(at: &AgentTerminal, browser: &Browser) {
    browser.run(&["scroll", "up", "3000", "--selector", "[aria-label='Calls to work app']"]);
    browser.wait_until(&format!("{FOLLOW}.ariaPressed === 'false'"));
    browser.wait_for_text("Jump to latest");
    let scrolled = browser.eval(&format!("{LIST}.scrollTop"));
    at.run(&["get", "frame"]).success();
    wait_for_calls(browser, 62);
    assert_eq!(browser.eval(&format!("{LIST}.scrollTop")), scrolled);
}

fn jump_to_latest_follows_again(browser: &Browser) {
    browser.click_button("Jump to latest");
    browser.wait_until(&at_bottom());
    assert_eq!(browser.eval(&format!("{FOLLOW}.ariaPressed")), "\"true\"");
    assert_eq!(browser.eval("document.activeElement.textContent"), "\"Follow\"");
    browser.wait_until(&format!("!{JUMP_SHOWN}"));
}

fn follow_toggles_and_keeps_new_calls_in_view(at: &AgentTerminal, browser: &Browser) {
    browser.click_button("Follow");
    browser.wait_until(&format!("{FOLLOW}.ariaPressed === 'false'"));
    browser.click_button("Follow");
    browser.wait_until(&format!("{FOLLOW}.ariaPressed === 'true'"));
    at.run(&["get", "frame"]).success();
    wait_for_calls(browser, 63);
    browser.wait_until(&at_bottom());
}

fn clearing_empties_the_list_until_the_next_call(at: &AgentTerminal, browser: &Browser) {
    browser.click_button("Clear activity");
    browser.wait_for_text("Waiting for calls...");
    assert_eq!(browser.eval("document.querySelector('#ppanel-activity [data-slot=badge]').textContent"), "\"0\"");
    assert_eq!(browser.eval("[...document.querySelectorAll('#ppanel-activity button')].find((b) => b.textContent === 'Clear activity').disabled"), "true");
    at.run(&["get", "frame"]).success();
    wait_for_calls(browser, 1);
    assert_eq!(
        calls(browser)[0]["parts"][0].as_str().unwrap().split(' ').take(2).collect::<Vec<_>>(),
        ["get", "frame"]
    );
    browser.select_panel("Output");
    browser.select_panel("Activity");
    wait_for_calls(browser, 1);
}

fn reopening_the_tab_starts_a_new_list(at: &AgentTerminal, browser: &Browser) {
    at.run(&["close"]).success();
    browser.wait_for_text("work app is not open");
    at.run(&["open", "--name", "app", "--size", "40x6", "--wait", "line 3", &fixture("line-printer"), "--hold"])
        .success();
    wait_for_calls(browser, 1);
    assert_eq!(calls(browser)[0]["parts"][0].as_str().unwrap().split(' ').next(), Some("open"));
}

#[test]
fn in_a_split_activity_shows_the_calls_to_the_focused_terminal() {
    let at = AgentTerminal::new();
    let printer = fixture("line-printer");
    for session in ["qa", "web"] {
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
            "line 3",
            &printer,
            "--hold",
        ])
        .success();
    }
    at.run(&["--session", "web", "get", "frame"]).success();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/split/qa:app,web:app"));
    browser
        .wait_until("document.querySelectorAll(\"[aria-label='Calls to qa app'] > li:not([aria-busy])\").length === 1");

    browser.run(&["click", "main section[aria-label='web app line-printer'] [role=group]"]);
    browser.wait_until(
        "document.querySelectorAll(\"[aria-label='Calls to web app'] > li:not([aria-busy])\").length === 2",
    );
    at.run(&["--session", "qa", "get", "frame"]).success();
    at.run(&["--session", "web", "get", "frame"]).success();
    browser.wait_until(
        "document.querySelectorAll(\"[aria-label='Calls to web app'] > li:not([aria-busy])\").length === 3",
    );
}

#[test]
fn clicking_an_activity_thumbnail_opens_the_lightbox_on_that_screenshot() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "40x6", "--wait", "line 3", &fixture("line-printer"), "--hold"])
        .success();
    at.run(&["screenshot", &at.home().join("shots").join("evidence.png").to_string_lossy()]).success();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/work/app"));

    browser.wait_until(
        "[...document.querySelectorAll(\"[aria-label='Calls to work app'] button\")].some((thumbnail) => thumbnail.innerText.trim() === 'Open screenshot evidence.png')",
    );
    browser.click_button("Open screenshot evidence.png");
    browser.wait_until("document.querySelector('[role=dialog] h2')?.innerText === 'evidence.png'");
}

#[test]
fn a_screenshot_call_shows_the_picture_it_saved_as_a_thumbnail() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "40x6", "--wait", "line 3", &fixture("line-printer"), "--hold"])
        .success();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/work/app"));
    wait_for_calls(&browser, 1);
    let shot = at.home().join("shots").join("evidence.png");
    at.run(&["screenshot", &shot.to_string_lossy()]).success();
    wait_for_calls(&browser, 2);

    let thumbnail = format!("document.querySelector(\"{CALLS}:nth-child(2) button img\")");
    browser.wait_until(&format!("{thumbnail}?.complete === true && {thumbnail}.naturalWidth > 0"));
    let png = std::fs::read(&shot).expect("the saved screenshot");
    let size = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap());
    assert_eq!(
        browser.eval(&format!("[{thumbnail}.naturalWidth, {thumbnail}.naturalHeight].join('x')")),
        format!("\"{}x{}\"", size(16), size(20))
    );
    browser.run(&["find", "role", "button", "hover", "--name", "Open screenshot evidence.png", "--exact"]);
    assert_eq!(browser.eval(&format!("document.querySelectorAll(\"{CALLS}:nth-child(1) img\").length")), "0");
}
