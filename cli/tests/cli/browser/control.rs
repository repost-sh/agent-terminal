use super::{Browser, Dashboard, js_string};
use crate::support::{AgentTerminal, fixture};

const REFUSED: &str = "A human has control of work app. Run `agent-terminal wait --control`.";

fn open_input_echo(at: &AgentTerminal) {
    at.run(&["open", "--name", "app", "--size", "40x6", "--wait", "ready", &fixture("input-echo")]).success();
}

fn take_control(browser: &Browser) {
    browser.wait_for_status("Live");
    browser.click_button("Take control");
    browser.wait_until("document.activeElement?.getAttribute('aria-label') === 'Terminal work app, you have control'");
}

fn rows(which: &str) -> String {
    format!(
        "[...document.querySelectorAll(\"[aria-label='Calls to work app'] > li{which}\")].map((row) => \
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
    let browser = Browser::open(&dashboard.url("/s/work/app"));
    take_control(&browser);

    the_dashboard_and_the_tab_show_the_human_has_control(&at, &browser);
    the_agent_is_refused_with_exit_4_and_told_to_wait(&at, &browser);
    releasing_control_ends_the_agents_wait(&at, &browser);
    the_agent_has_control_again(&at, &browser);
}

fn the_dashboard_and_the_tab_show_the_human_has_control(at: &AgentTerminal, browser: &Browser) {
    assert_eq!(control_button(browser), "\"Release control\"");
    browser.wait_until("document.querySelector('nav [aria-current=page]')?.innerText.includes('human')");
    assert!(
        browser.status().contains(&"Keys go to input-echo · ctrl+p ctrl+q moves focus out".to_string()),
        "{:?}",
        browser.status()
    );
    assert_eq!(at.run(&["--json", "tab"]).success().json()["tabs"][0]["control"], "human");
}

fn the_agent_is_refused_with_exit_4_and_told_to_wait(at: &AgentTerminal, browser: &Browser) {
    let refused = at.run(&["--session", "work", "--timeout", "500", "press", "j"]);
    assert_eq!(refused.code(), 4, "{}", refused.stderr());
    assert_eq!(refused.stderr(), format!("error: {REFUSED}\n"));
    wait_for_row(browser, ":not([aria-busy])", "--session work --timeout 500 press j ", "| refused: human has control");
    browser.wait_for_text("The agent is waiting for control");
    let notice = browser.text(".toaster");
    assert!(notice.lines().any(|line| line.trim() == "press j refused · exit 4 · now: wait --control"), "{notice}");
}

fn releasing_control_ends_the_agents_wait(at: &AgentTerminal, browser: &Browser) {
    std::thread::scope(|scope| {
        let waiting = scope.spawn(|| at.run(&["wait", "--control"]));
        wait_for_row(browser, "[aria-busy=true]", "wait --control", "waiting");
        browser.click_button("Release control");
        let waited = waiting.join().expect("the waiting call").success().stdout();
        assert!(waited.ends_with(" ms for a human to hand back control in work app\n"), "{waited}");
    });
}

fn the_agent_has_control_again(at: &AgentTerminal, browser: &Browser) {
    browser.wait_until("document.activeElement?.textContent === 'Take control'");
    browser.wait_until("!document.querySelector('nav [aria-current=page]')?.innerText.includes('human')");
    wait_for_row(browser, ":not([aria-busy])", "wait --control ", "");
    assert_eq!(browser.eval(&format!("{}.length", rows("[aria-busy]"))), "0");
    assert_eq!(at.run(&["--json", "tab"]).success().json()["tabs"][0]["control"], "agent");
    at.run(&["press", "j", "--wait", "j"]).success();
}

#[test]
fn keys_typed_in_the_dashboard_reach_the_program_and_show_in_activity_as_human_rows() {
    let at = AgentTerminal::new();
    open_input_echo(&at);
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/work/app"));
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
    assert_eq!(at.received("work"), r"hi\t\e\x03\e[Az");
}

#[test]
fn a_human_attached_from_a_terminal_shows_as_human_in_the_sidebar() {
    let at = AgentTerminal::new();
    open_input_echo(&at);
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/work/app"));
    browser.wait_for_status("Live");
    let binary = env!("CARGO_BIN_EXE_agent-terminal");
    let attach = ["--session", "work", "attach"];
    let mut args = vec![
        "--session",
        "human",
        "open",
        "--name",
        "app",
        "--size",
        "70x10",
        "--wait",
        "ctrl-p ctrl-q detaches",
        binary,
        "--",
    ];
    args.extend(attach);
    at.run(&["session", "new", "human"]).success();
    at.run(&args).success();
    browser.wait_until(
        "[...document.querySelectorAll('nav a')].some((a) => a.innerText.includes('input-echo') && a.innerText.includes('human'))",
    );
    assert_eq!(control_button(&browser), "\"Take control\"");
}
