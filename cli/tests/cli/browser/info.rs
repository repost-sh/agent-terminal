use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

use super::{Browser, Dashboard};
use crate::support::{AgentTerminal, Cue, fixture};

fn lists(browser: &Browser) -> Vec<Vec<(String, String)>> {
    let lists = browser.eval(
        "[...document.querySelectorAll('#ppanel-info dl')].map((list) => \
            [...list.querySelectorAll('dt')].map((term) => [term.textContent, term.nextElementSibling.textContent.trim()]))",
    );
    serde_json::from_str(&lists).expect("the Info lists")
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(key, value)| (key.to_string(), value.to_string())).collect()
}

#[test]
fn info_shows_the_setup_modes_recording_and_the_environment_with_secrets_masked_by_the_daemon() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    let printer = fixture("line-printer");
    let cwd = at.home().to_string_lossy().into_owned();
    let began = SystemTime::now() - Duration::from_secs(1);
    at.run(&[
        "open",
        "--size",
        "40x6",
        "--cwd",
        &cwd,
        "--env",
        "DEPLOY_TOKEN=hunter2-secret",
        "--env",
        "GREETING=hello there",
        "--wait",
        "line 1",
        &printer,
        "--lines",
        "1",
        "--cue",
        &cue.path(),
    ])
    .success();
    cue.print("");
    let pid = at.run(&["get", "pid"]).success().stdout().trim().to_string();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    browser.select_panel("Info");
    browser.wait_for_text("xterm-256color");

    let shown = lists(&browser);
    let args = format!("--lines 1 --cue {}", cue.path());
    let started = shown[0][8].1.clone();
    assert_eq!(
        shown[0],
        pairs(&[
            ("program", &printer),
            ("args", &args),
            ("pid", &pid),
            ("cwd", &cwd),
            ("TERM", "xterm-256color"),
            ("size", "40 × 6"),
            ("shell integration", "not a shell"),
            ("recording", "off"),
            ("started", &started),
        ])
    );
    let at_time: String =
        serde_json::from_str(&browser.eval("document.querySelector('#ppanel-info time').dateTime")).unwrap();
    let started_at = humantime::parse_rfc3339(&at_time).expect("an RFC 3339 start time");
    assert!(began <= started_at && started_at <= SystemTime::now(), "{at_time}");
    assert_eq!(started.len(), "Oct 14, 2026 14:02:07".len(), "{started}");
    assert_eq!(&started[started.len() - 2..], &at_time[17..19]);
    let off = pairs(&[
        ("alt-screen", "off"),
        ("mouse tracking", "off"),
        ("bracketed paste", "off"),
        ("application cursor keys", "off"),
        ("focus events", "off"),
        ("kitty keyboard flags", "off"),
    ]);
    assert_eq!(shown[1], off);

    cue.print("\x1b[?1049h\x1b[?1000h\x1b[?1006h\x1b[?2004h\x1b[?1h\x1b[?1004h");
    browser.wait_for_text("on (sgr)");
    assert_eq!(
        lists(&browser)[1],
        pairs(&[
            ("alt-screen", "on"),
            ("mouse tracking", "on (sgr)"),
            ("bracketed paste", "on"),
            ("application cursor keys", "on"),
            ("focus events", "on"),
            ("kitty keyboard flags", "off"),
        ])
    );

    let cast = at.home().join("evidence.cast");
    at.run(&["record", "start", &cast.to_string_lossy()]).success();
    browser.wait_for_text(&cast.to_string_lossy());
    at.run(&["record", "stop"]).success();
    browser.wait_until("[...document.querySelectorAll('#ppanel-info dt')].find((term) => term.textContent === 'recording').nextElementSibling.textContent === 'off'");
    at.run(&["resize", "50x8"]).success();
    browser.wait_until(
        "[...document.querySelectorAll('#ppanel-info dt')].find((term) => term.textContent === 'size').nextElementSibling.textContent === '50 × 8'",
    );

    assert_eq!(browser.eval("document.querySelector('#ppanel-info details').open"), "false");
    browser.run(&["click", "#ppanel-info summary"]);
    browser.wait_until("document.querySelector('#ppanel-info details').open");
    let environment = lists(&browser).remove(2);
    let summary: String =
        serde_json::from_str(&browser.eval("document.querySelector('#ppanel-info summary').textContent")).unwrap();
    assert_eq!(summary, format!("Environment ({})", environment.len()));
    let value = |name: &str| environment.iter().find(|(key, _)| key == name).map(|(_, value)| value.clone());
    assert_eq!(value("GREETING").as_deref(), Some("hello there"));
    assert_eq!(value("TERM").as_deref(), Some("xterm-256color"));
    assert_eq!(value("COLORTERM").as_deref(), Some("truecolor"));
    assert_eq!(value("DEPLOY_TOKEN").as_deref(), Some("••••masked"));
    let names: Vec<&String> = environment.iter().map(|(key, _)| key).collect();
    assert!(names.is_sorted(), "{names:?}");

    let sent: Value = serde_json::from_str(&browser.eval(
        "new Promise((resolve) => { const socket = new WebSocket(`ws://${location.host}/api/session/default/t1/info`); \
            socket.onmessage = (event) => { resolve(event.data); socket.close(); }; })",
    ))
    .unwrap();
    let sent = sent.as_str().expect("the Info message");
    assert!(!sent.contains("hunter2-secret"), "{sent}");
    let sent: Value = serde_json::from_str(sent).unwrap();
    let token = sent["env"].as_array().unwrap().iter().find(|var| var["name"] == "DEPLOY_TOKEN").cloned();
    assert_eq!(token, Some(json!({ "name": "DEPLOY_TOKEN", "value": null })));
    assert!(!browser.eval("document.documentElement.outerHTML").contains("hunter2-secret"));
}
