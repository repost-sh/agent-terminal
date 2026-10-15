use super::{Browser, Dashboard, js_string};
use crate::support::{AgentTerminal, Cue, fixture};

const STREAM: &str = "[aria-label='Output of default t1'] > *";

fn stream(browser: &Browser) -> Vec<String> {
    let lines =
        browser.eval(&format!("[...document.querySelectorAll({})].map((line) => line.textContent)", js_string(STREAM)));
    serde_json::from_str(&lines).expect("the Output lines")
}

fn wait_for_output(browser: &Browser, text: &str) {
    browser.wait_until(&format!(
        "document.querySelector(\"[aria-label='Output of default t1']\")?.textContent.includes({})",
        js_string(text)
    ));
}

fn open_output(browser: &Browser) {
    browser.select_panel("Output");
}

#[test]
fn output_shows_the_text_stream_with_marks_as_dividers_escapes_on_raw_and_search_with_a_count() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    at.run(&[
        "open",
        "--size",
        "40x6",
        "--wait",
        "line 2",
        &fixture("line-printer"),
        "--lines",
        "2",
        "--cue",
        &cue.path(),
    ])
    .success();
    cue.print("");
    at.run(&["mark", "before-alert"]).success();
    cue.print("\x1b[31mred alert\x1b[0m\n");
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    open_output(&browser);

    wait_for_output(&browser, "red alert");
    assert_eq!(stream(&browser), ["line 1", "line 2", "mark: before-alert", "red alert"]);
    assert_eq!(at.run(&["output"]).success().stdout(), "line 1\nline 2\nred alert\n");
    assert_eq!(
        browser.eval(&format!(
            "getComputedStyle([...document.querySelectorAll({} + ' span')].find((span) => span.textContent === 'red alert')).color",
            js_string(STREAM)
        )),
        "\"rgb(248, 113, 113)\""
    );
    assert_eq!(
        browser.eval("document.querySelector('[role=separator][aria-label=\"mark before-alert\"]') !== null"),
        "true"
    );

    cue.print("alert again\n");
    wait_for_output(&browser, "alert again");
    assert_eq!(stream(&browser), ["line 1", "line 2", "mark: before-alert", "red alert", "alert again"]);

    browser.run(&["fill", "input[placeholder='Search']", "alert"]);
    browser.wait_for_text("2 matches");
    assert_eq!(browser.eval(&format!("document.querySelectorAll({} + ' mark').length", js_string(STREAM))), "2");

    browser.click_button("Raw");
    browser.wait_until("document.querySelector('#ppanel-output button[aria-pressed]').ariaPressed === 'true'");
    browser.run(&["fill", "input[placeholder='Search']", "␛[6n"]);
    browser.wait_for_text("3 matches");
    browser.run(&["fill", "input[placeholder='Search']", " "]);
    browser.wait_until("!document.querySelector('#ppanel-output [role=status]')?.textContent.includes('match')");
    assert_eq!(
        stream(&browser),
        ["line 1␍␊", "line 2␍␊", "␛[6n", "mark: before-alert", "␛[31mred alert␛[0m␍␊", "␛[6nalert again␍␊", "␛[6n",]
    );
}

#[test]
fn output_of_a_tab_that_printed_nothing_says_so() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "40x6", &fixture("line-printer"), "--lines", "0", "--hold"]).success();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    open_output(&browser);

    browser.wait_for_text("No output yet");
}
