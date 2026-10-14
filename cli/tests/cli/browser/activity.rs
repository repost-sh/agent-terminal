use serde_json::Value;

use super::{Browser, Dashboard};
use crate::support::{AgentTerminal, fixture};

const CALLS: &str = "[aria-label='Calls to default t1'] > li";

fn logged(at: &AgentTerminal) -> Vec<Value> {
    std::fs::read_to_string(at.home().join("sessions/default/activity.jsonl"))
        .expect("the activity log")
        .lines()
        .map(|line| serde_json::from_str(line).expect("an activity entry"))
        .collect()
}

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
    if ms < 1000 { format!("{ms} ms") } else { format!("{:.1} s", ms as f64 / 1000.0) }
}

fn seconds_and_millis(time: &str) -> &str {
    &time[time.len() - 6..]
}

#[test]
fn activity_lists_every_call_to_the_tab_live_with_its_time_duration_and_result() {
    let at = AgentTerminal::new();
    let printer = fixture("line-printer");
    at.run(&["open", "--size", "40x6", "--wait", "line 3", &printer, "--hold"]).success();
    at.run(&["tab", "new", "--size", "40x6", "--wait", "line 3", &printer, "--hold"]).success();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    wait_for_calls(&browser, 1);

    at.run(&["--tab", "t1", "snapshot"]).success();
    assert_eq!(at.run(&["--tab", "t1", "--timeout", "300", "wait", "never shown"]).code(), 3);
    assert_eq!(at.run(&["--tab", "t1", "expect", "not there"]).code(), 2);
    assert_eq!(at.run(&["--tab", "t1", "get", "cell", "99", "1"]).code(), 1);
    at.run(&["--tab", "t1", "press", "Enter"]).success();
    at.run(&["--tab", "t1", "wait", "--exit"]).success();
    assert_eq!(at.run(&["--tab", "t1", "press", "y"]).code(), 5);
    wait_for_calls(&browser, 8);

    let log: Vec<Value> = logged(&at).into_iter().filter(|entry| entry["tab"] == "t1").collect();
    let shown = calls(&browser);
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
    for ((row, entry), result) in shown.iter().zip(&log).zip(results) {
        let time = entry["time"].as_str().unwrap();
        assert_eq!(row["at"], time);
        let text = row["time"].as_str().unwrap();
        assert_eq!(text.len(), "14:02:07.312".len(), "{row}");
        assert_eq!(seconds_and_millis(text), &time[17..23], "{row}");
        let mut parts = vec![format!(
            "{} {}",
            entry["command"].as_str().unwrap(),
            duration(entry["duration_ms"].as_u64().unwrap())
        )];
        parts.extend(result.map(str::to_string));
        parts.push("+".into());
        assert_eq!(row["parts"], serde_json::json!(parts), "{entry}");
    }
    assert_eq!(log[1]["command"], "--tab t1 snapshot");
    assert_eq!(browser.eval("document.querySelector('#ppanel-activity [data-slot=badge]').textContent"), "\"8\"");

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

#[test]
fn activity_follows_new_calls_until_scrolled_up_then_offers_jump_to_latest_and_clears() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "40x6", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    wait_for_calls(&browser, 1);
    let mut batch = vec!["batch"];
    batch.extend(std::iter::repeat_n("get frame", 60));
    at.run(&batch).success();
    wait_for_calls(&browser, 61);

    let list = "document.querySelector(\"[aria-label='Calls to default t1']\")";
    let at_bottom = format!(
        "(() => {{ const list = {list}; return list.scrollHeight - list.scrollTop - list.clientHeight < 2; }})()"
    );
    browser.wait_until(&at_bottom);
    let follow = "document.querySelector('#ppanel-activity button[aria-pressed]')";
    assert_eq!(browser.eval(&format!("{follow}.textContent + ' ' + {follow}.ariaPressed")), "\"Follow true\"");
    assert_eq!(
        browser.eval(
            "[...document.querySelectorAll('button')].some((b) => b.textContent === 'Jump to latest' && !b.hidden)"
        ),
        "false"
    );

    browser.run(&["scroll", "up", "3000", "--selector", "[aria-label='Calls to default t1']"]);
    browser.wait_until(&format!("{follow}.ariaPressed === 'false'"));
    browser.wait_for_text("Jump to latest");
    let scrolled = browser.eval(&format!("{list}.scrollTop"));
    at.run(&["get", "frame"]).success();
    wait_for_calls(&browser, 62);
    assert_eq!(browser.eval(&format!("{list}.scrollTop")), scrolled);

    browser.click_button("Jump to latest");
    browser.wait_until(&at_bottom);
    assert_eq!(browser.eval(&format!("{follow}.ariaPressed")), "\"true\"");
    assert_eq!(browser.eval("document.activeElement.textContent"), "\"Follow\"");
    browser.wait_until(
        "![...document.querySelectorAll('button')].some((b) => b.textContent === 'Jump to latest' && !b.hidden)",
    );

    browser.click_button("Follow");
    browser.wait_until(&format!("{follow}.ariaPressed === 'false'"));
    browser.click_button("Follow");
    browser.wait_until(&format!("{follow}.ariaPressed === 'true'"));
    at.run(&["get", "frame"]).success();
    wait_for_calls(&browser, 63);
    browser.wait_until(&at_bottom);

    browser.click_button("Clear activity");
    browser.wait_for_text("Waiting for calls...");
    assert_eq!(browser.eval("document.querySelector('#ppanel-activity [data-slot=badge]').textContent"), "\"0\"");
    assert_eq!(browser.eval("[...document.querySelectorAll('#ppanel-activity button')].find((b) => b.textContent === 'Clear activity').disabled"), "true");
    at.run(&["get", "frame"]).success();
    wait_for_calls(&browser, 1);
    assert_eq!(
        calls(&browser)[0]["parts"][0].as_str().unwrap().split(' ').take(2).collect::<Vec<_>>(),
        ["get", "frame"]
    );
}
