use super::{Browser, Dashboard, js_string};
use crate::support::{AgentTerminal, fixture};

const REFUSED: &str = "A human has control of default t1. Run `agent-terminal wait --control`.";
const CONTROL_BUTTON: &str = "#take-control-default-t1";
const TIMELINE: &str = "main input[type=range][aria-label='Timeline of default t1']";
const PAST: &str = "#viewport-region [aria-label*=', viewing frame ']";

fn open_input_echo(at: &AgentTerminal) {
    at.run(&["open", "--size", "40x6", "--wait", "ready", &fixture("input-echo")]).success();
}

fn take_control(browser: &Browser) {
    browser.wait_for_status("Live");
    browser.click_button("Take control");
    browser
        .wait_until("document.activeElement?.getAttribute('aria-label') === 'Terminal default t1, you have control'");
}

fn rows(which: &str) -> String {
    format!(
        "[...document.querySelectorAll(\"[aria-label='Calls to default t1'] > li{which}\")].map((row) => \
            row.innerText.split('\\n').map((part) => part.trim()).filter(Boolean).slice(1).join(' | '))"
    )
}

fn wait_for_row(browser: &Browser, which: &str, starts: &str, then: &str) {
    browser.wait_until(&format!(
        "{}.some((row) => row.startsWith({}) && row.includes({}))",
        rows(which),
        js_string(starts),
        js_string(then)
    ));
}

fn human_rows(browser: &Browser) -> Vec<String> {
    let rows: Vec<String> = serde_json::from_str(&browser.eval(&rows(":not([aria-busy])"))).expect("the Activity rows");
    rows.into_iter().filter(|row| row.starts_with("human:")).collect()
}

fn control_button(browser: &Browser) -> String {
    browser.eval(
        "[...document.querySelectorAll('main button')].find((b) => b.textContent.endsWith(' control'))?.textContent",
    )
}

#[test]
fn taking_control_refuses_the_agent_with_exit_4_until_the_human_releases_it() {
    let at = AgentTerminal::new();
    open_input_echo(&at);
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    take_control(&browser);

    assert_eq!(control_button(&browser), "\"Release control\"");
    browser.wait_until("document.querySelector('nav [aria-current=page]')?.innerText.includes('human')");
    assert!(
        browser.status().contains(&"Keys go to input-echo · ctrl+p ctrl+q moves focus out".to_string()),
        "{:?}",
        browser.status()
    );
    assert_eq!(at.run(&["--json", "tab"]).success().json()["tabs"][0]["control"], "human");

    let refused = at.run(&["press", "j"]);
    assert_eq!(refused.code(), 4, "{}", refused.stderr());
    assert_eq!(refused.stderr(), format!("error: {REFUSED}\n"));
    wait_for_row(&browser, ":not([aria-busy])", "press j ", "| refused: human has control");
    browser.wait_for_text("The agent is waiting for control");
    let notice = browser.text(".toaster");
    assert!(notice.contains("press j refused · exit 4 · now: wait --control"), "{notice}");

    std::thread::scope(|scope| {
        let waiting = scope.spawn(|| at.run(&["wait", "--control"]));
        wait_for_row(&browser, "[aria-busy=true]", "wait --control", "waiting");
        browser.click_button("Release control");
        let waited = waiting.join().expect("the waiting call").success().stdout();
        assert!(waited.ends_with(" ms for a human to hand back control in default t1\n"), "{waited}");
    });
    browser.wait_until("document.activeElement?.textContent === 'Take control'");
    browser.wait_until("!document.querySelector('nav [aria-current=page]')?.innerText.includes('human')");
    wait_for_row(&browser, ":not([aria-busy])", "wait --control ", "");
    assert_eq!(browser.eval(&format!("{}.length", rows("[aria-busy]"))), "0");
    assert_eq!(at.run(&["--json", "tab"]).success().json()["tabs"][0]["control"], "agent");
    at.run(&["press", "j", "--wait", "j"]).success();
}

#[test]
fn keys_typed_in_the_dashboard_reach_the_program_and_show_in_activity_as_human_rows() {
    let at = AgentTerminal::new();
    open_input_echo(&at);
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    take_control(&browser);

    for key in ["h", "i", "Tab", "Escape", "Control+c", "ArrowUp"] {
        browser.run(&["press", key]);
    }
    at.run(&["wait", r"hi\t\e\x03\e[A"]).success();
    wait_for_row(&browser, ":not([aria-busy])", "human: pressed Up", "");
    assert_eq!(
        human_rows(&browser),
        [
            "human: typed \"hi\"",
            "human: pressed Tab",
            "human: pressed Esc",
            "human: pressed Ctrl+c",
            "human: pressed Up"
        ]
    );

    browser.run(&["press", "Control+p"]);
    browser.run(&["press", "Control+q"]);
    browser.wait_until("document.activeElement?.textContent === 'Release control'");
    browser.run(&["focus", "[role=application]"]);
    browser.run(&["press", "z"]);
    at.run(&["wait", r"\e[Az"]).success();
    assert_eq!(at.received("default"), r"hi\t\e\x03\e[Az");
}

#[test]
fn a_human_attached_from_a_terminal_shows_as_human_in_the_sidebar() {
    let at = AgentTerminal::new();
    open_input_echo(&at);
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    browser.wait_for_status("Live");
    let binary = env!("CARGO_BIN_EXE_agent-terminal");
    let attach = ["--session", "default", "attach"];
    let mut args = vec!["--session", "human", "open", "--size", "70x10", "--wait", "ctrl-p ctrl-q detaches", binary];
    args.extend(attach);
    at.run(&args).success();
    browser.wait_until(
        "[...document.querySelectorAll('nav a')].some((a) => a.innerText.includes('input-echo') && a.innerText.includes('human'))",
    );
    assert_eq!(control_button(&browser), "\"Take control\"");
}

#[test]
fn keys_typed_while_viewing_the_past_do_not_reach_the_program_and_reach_it_again_once_live() {
    let at = AgentTerminal::new();
    open_input_echo(&at);
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    take_control(&browser);
    browser.run(&["press", "k"]);
    at.run(&["wait", "k"]).success();
    let frame = at.run(&["get", "frame"]).success().stdout().trim().to_string();
    browser.wait_until(&format!(
        "(() => {{ const range = document.querySelector({}); return !!range && !range.disabled && range.max === '{frame}'; }})()",
        js_string(TIMELINE)
    ));

    browser.run(&["focus", TIMELINE]);
    browser.run(&["press", "Home"]);
    browser.wait_until(&format!(
        "document.querySelector({})?.ariaLabel.endsWith(', viewing frame 1') ?? false",
        js_string(PAST)
    ));
    browser.wait_for_screen_in(PAST, "ready");
    browser.run(&["focus", CONTROL_BUTTON]);
    browser.run(&["press", "Tab"]);
    browser.run(&["press", "Enter"]);

    browser.click_button("Live");
    browser.wait_until(&format!("!document.querySelector({})", js_string(PAST)));
    browser.run(&["focus", CONTROL_BUTTON]);
    browser.run(&["press", "Tab"]);
    browser.run(&["press", "z"]);
    at.run(&["wait", "z"]).success();
    assert_eq!(at.received("default"), "kz");
}
