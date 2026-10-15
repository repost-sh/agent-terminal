use super::{Browser, Dashboard, js_string, snapshot_rows};
use crate::support::{AgentTerminal, Cue, fixture};

const TIMELINE: &str = "main input[type=range][aria-label='Timeline of default t1']";
const PAST: &str = "#viewport-region [aria-label*=', viewing frame ']";
const CALLS: &str = "ol[aria-label='Calls to default t1']";

fn open_printer(at: &AgentTerminal, cue: &Cue) {
    at.run(&[
        "open",
        "--size",
        "40x6",
        "--wait",
        "line 1",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--cue",
        &cue.path(),
    ])
    .success();
}

fn stream(dashboard: &Dashboard) -> String {
    format!("ws://localhost:{}/api/session/default/t1/stream", dashboard.port)
}

fn wait_for_timeline(browser: &Browser, last: u64) {
    browser.wait_until(&format!(
        "(() => {{ const range = document.querySelector({}); return !!range && !range.disabled && range.max === '{last}'; }})()",
        js_string(TIMELINE)
    ));
}

fn wait_for_past(browser: &Browser, frame: u64, rows: &[String]) {
    browser.wait_until(&format!(
        "(() => {{ const shown = document.querySelector({}); if (!shown || !shown.ariaLabel.endsWith(', viewing frame {frame}')) return false; const tree = shown.querySelector('.xterm-accessibility-tree'); if (!tree) return false; const rows = [...tree.children].map((row) => row.textContent.trimEnd()); while (rows.length && !rows[rows.length - 1]) rows.pop(); return JSON.stringify(rows) === {}; }})()",
        js_string(PAST),
        js_string(&serde_json::to_string(rows).expect("rows as JSON"))
    ));
    assert_eq!(browser.screen_in(PAST), rows, "frame {frame}");
}

fn assert_viewing(browser: &Browser, dashboard: &Dashboard, frame: u64) {
    let status = browser.status();
    let label = regex::Regex::new(&format!(r"^Viewing frame {frame} · (\d{{2}}:\d{{2}}:\d{{2}})$")).unwrap();
    let time = label.captures(&status[0]).unwrap_or_else(|| panic!("{status:?}"))[1].to_string();
    assert_eq!(status[1..], [stream(dashboard), "40 × 6".to_string()]);
    assert!(!live_pressed(browser));
    let described = browser.eval(&format!("document.querySelector({}).ariaValueText", js_string(TIMELINE)));
    assert_eq!(described, js_string(&format!("frame {frame} · {time}")));
}

fn pressed(browser: &Browser, scope: &str, name: &str) -> bool {
    let found = browser.eval(&format!(
        "[...document.querySelectorAll('{scope} button')].find((button) => button.textContent === '{name}')?.ariaPressed ?? 'missing'"
    ));
    match found.as_str() {
        "\"true\"" => true,
        "\"false\"" => false,
        other => panic!("the {name} button is {other}"),
    }
}

fn live_pressed(browser: &Browser) -> bool {
    pressed(browser, "main", "Live")
}

fn follow_pressed(browser: &Browser) -> bool {
    pressed(browser, "body", "Follow")
}

fn wait_for_live(browser: &Browser, frame: u64, rows: &[String]) {
    browser.wait_for_status(&format!("frame {frame}"));
    browser.wait_for_status("Live");
    browser.wait_until(&format!("!document.querySelector({})", js_string(PAST)));
    assert_eq!(browser.screen(), rows);
    assert!(live_pressed(browser));
}

fn ticks(browser: &Browser, kind: &str) -> u64 {
    browser.eval(&format!("document.querySelectorAll('main [data-tick={kind}]').length")).parse().expect("a count")
}

#[test]
fn the_keyboard_and_a_drag_show_past_frames_exactly_as_snapshot_showed_them_and_live_returns() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    open_printer(&at, &cue);
    let first = snapshot_rows(&at, &[]);
    cue.print("second\n");
    let second = snapshot_rows(&at, &[]);
    cue.print("third\n");
    let third = snapshot_rows(&at, &[]);
    assert_eq!(first, (vec!["line 1".to_string()], 1));
    assert_eq!(third.0, ["line 1", "second", "third"]);

    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    browser.wait_for_status(&format!("frame {}", third.1));
    wait_for_timeline(&browser, third.1);
    assert!(live_pressed(&browser));

    browser.run(&["focus", TIMELINE]);
    browser.run(&["press", "Home"]);
    wait_for_past(&browser, 1, &first.0);
    assert_viewing(&browser, &dashboard, 1);

    for _ in 1..second.1 {
        browser.run(&["press", "ArrowRight"]);
    }
    wait_for_past(&browser, second.1, &second.0);
    assert_viewing(&browser, &dashboard, second.1);

    browser.run(&["press", "End"]);
    wait_for_live(&browser, third.1, &third.0);
    assert_eq!(browser.status(), ["Live", &stream(&dashboard), "40 × 6", &format!("frame {}", third.1)]);

    let found = browser.run(&["get", "box", TIMELINE]);
    let numbers: Vec<f64> = regex::Regex::new(r"-?\d+(\.\d+)?")
        .unwrap()
        .find_iter(&found)
        .map(|number| number.as_str().parse().expect("a number"))
        .collect();
    let (x, y, width, height) = (numbers[0], numbers[1], numbers[2], numbers[3]);
    let pixel = |at: f64| format!("{}", at.round() as i64);
    let middle = pixel(y + height / 2.0);
    browser.run(&["mouse", "move", &pixel(x + width - 5.0), &middle]);
    browser.run(&["mouse", "down"]);
    browser.run(&["mouse", "move", &pixel(x + width / 2.0), &middle]);
    browser.run(&["mouse", "move", &pixel(x + 1.0), &middle]);
    browser.run(&["mouse", "up"]);
    wait_for_past(&browser, 1, &first.0);
    assert_viewing(&browser, &dashboard, 1);

    cue.print("fourth\n");
    let fourth = snapshot_rows(&at, &[]);
    wait_for_timeline(&browser, fourth.1);
    assert_eq!(browser.screen_in(PAST), first.0);
    assert_viewing(&browser, &dashboard, 1);

    browser.click_button("Live");
    wait_for_live(&browser, fourth.1, &fourth.0);
}

#[test]
fn clicking_an_activity_row_shows_its_frame_and_stops_following_and_follow_does_not_go_live() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    open_printer(&at, &cue);
    cue.print("second\n");
    let second = snapshot_rows(&at, &[]);
    at.run(&["screenshot", &at.home().join("second.png").to_string_lossy()]).success();
    cue.print("third\n");
    let third = snapshot_rows(&at, &[]);

    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    browser.wait_for_status(&format!("frame {}", third.1));
    wait_for_timeline(&browser, third.1);
    browser.wait_until(&format!("document.querySelectorAll(\"{CALLS} > li:not([aria-busy])\").length === 4"));
    browser.wait_until("document.querySelectorAll('main [data-tick=call]').length === 4");
    assert_eq!((ticks(&browser, "call"), ticks(&browser, "command"), ticks(&browser, "shot")), (4, 0, 1));
    assert!(follow_pressed(&browser));

    browser.run(&["focus", TIMELINE]);
    browser.run(&["press", "Home"]);
    wait_for_past(&browser, 1, &["line 1".to_string()]);
    assert!(follow_pressed(&browser), "scrubbing leaves Follow as it was");
    browser.run(&["press", "End"]);
    wait_for_live(&browser, third.1, &third.0);
    assert!(follow_pressed(&browser));

    let second_row = format!("{CALLS} > li:not([aria-busy]):nth-child(2) button");
    browser.run(&["click", &second_row]);
    wait_for_past(&browser, second.1, &second.0);
    assert_viewing(&browser, &dashboard, second.1);
    assert!(!follow_pressed(&browser), "opening a row stops following");
    assert_eq!(browser.eval(&format!("document.querySelector(\"{second_row}\").ariaExpanded")), "\"true\"");

    browser.click_button("Follow");
    assert!(follow_pressed(&browser));
    assert_viewing(&browser, &dashboard, second.1);

    browser.run(&["click", &format!("{CALLS} > li:not([aria-busy]):nth-child(4) button")]);
    wait_for_live(&browser, third.1, &third.0);
}

#[test]
fn clicking_a_command_shows_the_terminal_where_it_started_and_split_view_has_no_timeline() {
    let at = AgentTerminal::new();
    let home = at.home().join("user");
    std::fs::create_dir_all(&home).expect("create a home folder");
    at.run_with_env(&["shell", "--shell", "bash", "--size", "60x12"], &[("HOME", home.as_path())]).success();
    at.run(&["exec", "echo $((40 + 2))"]).success();
    at.run(&["exec", "echo done"]).success();
    let (rows, frame) = snapshot_rows(&at, &[]);
    assert!(rows.iter().any(|row| row == "42"), "{rows:?}");
    let started =
        at.run(&["--json", "history", "show", "1"]).success().json()["first_frame"].as_u64().expect("a frame");

    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    browser.wait_for_status(&format!("frame {frame}"));
    browser.wait_until(&format!(
        "(() => {{ const range = document.querySelector({}); return !!range && range.max === '{frame}'; }})()",
        js_string(TIMELINE)
    ));
    browser.wait_until("document.querySelectorAll('main [data-tick=command]').length === 2");

    browser.select_panel("Commands");
    let first_command = "ol[aria-label='Commands in default t1'] > li:nth-child(1) button";
    browser.wait_until(&format!("!!document.querySelector({})", js_string(first_command)));
    browser.run(&["click", first_command]);
    browser.wait_until(&format!(
        "document.querySelector({})?.ariaLabel.endsWith(', viewing frame {started}') ?? false",
        js_string(PAST)
    ));
    assert!(browser.status()[0].starts_with(&format!("Viewing frame {started} · ")), "{:?}", browser.status());
    browser.wait_until(&format!(
        "document.querySelector({})?.querySelector('.xterm-accessibility-tree')?.textContent.includes('$') ?? false",
        js_string(PAST)
    ));
    assert!(!browser.screen_in(PAST).iter().any(|row| row == "42"), "{:?}", browser.screen_in(PAST));

    browser.click_button("Two terminals side by side");
    browser.wait_until("location.pathname.startsWith('/split/')");
    browser.wait_until("!!document.querySelector('.split-grid .xterm-accessibility-tree')");
    assert_eq!(browser.eval("document.querySelectorAll('main input[type=range]').length"), "0");
    assert_eq!(browser.eval(&format!("document.querySelector({}) === null", js_string(PAST))), "true");
}

#[test]
fn a_toolbar_screenshot_saves_the_frame_on_view_and_ticks_the_timeline_and_human_keys_tick_nothing() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "40x6", "--wait", "ready", &fixture("input-echo")]).success();
    at.run(&["send", "a", "--wait", "a"]).success();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    browser.wait_for_status("Live");
    wait_for_timeline(&browser, 2);
    browser.wait_until("document.querySelectorAll('main [data-tick=call]').length === 2");

    browser.click_button("Take control");
    browser
        .wait_until("document.activeElement?.getAttribute('aria-label') === 'Terminal default t1, you have control'");
    browser.run(&["press", "b"]);
    at.run(&["wait", "ab"]).success();
    browser.wait_until(&format!("document.querySelectorAll(\"{CALLS} > li:not([aria-busy])\").length === 4"));
    wait_for_timeline(&browser, 3);
    assert_eq!(ticks(&browser, "call"), 3, "open, send and wait are calls, the human key is not");
    browser.click_button("Release control");

    browser.run(&["focus", TIMELINE]);
    browser.run(&["press", "Home"]);
    wait_for_past(&browser, 1, &["ready".to_string()]);
    browser.click_button("Take a screenshot");
    browser.wait_until("document.querySelectorAll('main [data-tick=shot]').length === 1");
    let listed =
        std::fs::read_to_string(at.home().join("sessions/default/screenshots.jsonl")).expect("the screenshot list");
    let shot: serde_json::Value = serde_json::from_str(listed.lines().last().expect("a listed screenshot")).unwrap();
    assert_eq!((&shot["frame"], &shot["by"]), (&serde_json::json!(1), &serde_json::Value::Null));
    assert_eq!(shot["path"], at.home().join("screenshots/default-t1-1.png").to_string_lossy().as_ref());
}
