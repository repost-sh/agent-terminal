use std::path::PathBuf;

use super::{Browser, Dashboard, js_string};
use crate::support::AgentTerminal;

const GALLERY: &str = "ul[aria-label='Screenshots of default']";
const LIGHTBOX: &str = "[role=dialog]";
const LINES: &str = "stty -echo; printf '\\033[?25lone'; read _; printf '\\ntwo'; read _";

struct Shots {
    at: AgentTerminal,
    dir: PathBuf,
}

impl Shots {
    fn opened() -> Self {
        let at = AgentTerminal::new();
        at.run(&["open", "--size", "20x4", "--wait", "one", "sh", "-c", LINES]).success();
        let dir = at.home().join("shots");
        Self { at, dir }
    }

    fn path(&self, name: &str) -> String {
        self.dir.join(name).to_string_lossy().into_owned()
    }

    fn frame(&self) -> u64 {
        self.at.run(&["get", "frame"]).success().stdout().trim().parse().expect("a frame number")
    }

    fn before(&self) -> u64 {
        self.at.run(&["screenshot", &self.path("before.png")]).success();
        self.frame()
    }

    fn after(&self) -> u64 {
        self.at.run(&["press", "Enter", "--wait", "two", "--screenshot", &self.path("after.png")]).success();
        self.frame()
    }
}

fn open_screenshots(browser: &Browser) {
    browser.select_panel("Screenshots");
}

fn wait_for_shots(browser: &Browser, count: usize) {
    browser.wait_until(&format!(
        "document.querySelectorAll({}).length === {count}",
        js_string(&format!("{GALLERY} > li"))
    ));
}

fn eval_string(browser: &Browser, script: &str) -> String {
    let value = browser.eval(script);
    serde_json::from_str(&value).unwrap_or_else(|e| panic!("{script} gave {value}, not a string: {e}"))
}

fn without_time(text: &str) -> String {
    regex::Regex::new(r"\d{2}:\d{2}:\d{2}").unwrap().replace_all(text, "<time>").into_owned()
}

fn gallery(browser: &Browser) -> Vec<Vec<String>> {
    let script = format!(
        "JSON.stringify([...document.querySelectorAll({})].map((shot) => shot.innerText.split('\\n').map((part) => part.trim()).filter(Boolean)))",
        js_string(&format!("{GALLERY} > li > button"))
    );
    let shots: Vec<Vec<String>> = serde_json::from_str(&eval_string(browser, &script)).expect("the gallery as JSON");
    shots.into_iter().map(|parts| parts.iter().map(|part| without_time(part)).collect()).collect()
}

fn thumbnail_width(browser: &Browser, nth: usize) -> String {
    let image = format!("document.querySelector({})", js_string(&format!("{GALLERY} > li:nth-child({nth}) img")));
    browser.wait_until(&format!("{image}?.complete === true && {image}.naturalWidth > 0"));
    browser.eval(&format!("{image}.naturalWidth"))
}

fn open_shot(browser: &Browser, nth: usize) {
    browser.run(&["click", &format!("{GALLERY} > li:nth-child({nth}) > button")]);
    browser.wait_until(&format!("document.querySelector({}) !== null", js_string(LIGHTBOX)));
}

fn title(browser: &Browser) -> String {
    eval_string(browser, &format!("document.querySelector({}).innerText", js_string(&format!("{LIGHTBOX} h2"))))
}

fn wait_for_title(browser: &Browser, text: &str) {
    browser.wait_until(&format!(
        "document.querySelector({})?.innerText === {}",
        js_string(&format!("{LIGHTBOX} h2")),
        js_string(text)
    ));
}

fn captions(browser: &Browser) -> Vec<String> {
    let script = format!(
        "JSON.stringify([...document.querySelectorAll({})].map((caption) => caption.innerText))",
        js_string(&format!("{LIGHTBOX} figcaption"))
    );
    let captions: Vec<String> = serde_json::from_str(&eval_string(browser, &script)).expect("captions as JSON");
    captions.iter().map(|caption| without_time(caption)).collect()
}

fn button_state(browser: &Browser, name: &str) -> String {
    eval_string(
        browser,
        &format!(
            "(() => {{ const button = [...document.querySelectorAll({})].find((b) => b.innerText.trim() === {}); \
             return button ? (button.disabled ? 'disabled' : 'enabled') : 'missing'; }})()",
            js_string(&format!("{LIGHTBOX} button")),
            js_string(name)
        ),
    )
}

fn focused(browser: &Browser) -> String {
    eval_string(browser, "document.activeElement.innerText?.trim() || document.activeElement.tagName")
}

fn focus_is(browser: &Browser, selector: &str) -> bool {
    browser.eval(&format!("document.activeElement === document.querySelector({})", js_string(selector))) == "true"
}

#[test]
fn the_screenshots_tab_lists_each_screenshot_of_the_session_live_with_the_call_that_took_it() {
    let shots = Shots::opened();
    let dashboard = Dashboard::start(&shots.at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    open_screenshots(&browser);
    browser.wait_for_text("No screenshots yet");
    assert_eq!(browser.text("[role=tabpanel] [data-slot=badge]").trim(), "0");

    let before = shots.before();
    wait_for_shots(&browser, 1);
    let after = shots.after();
    wait_for_shots(&browser, 2);

    let (before_path, after_path) = (shots.path("before.png"), shots.path("after.png"));
    assert_eq!(
        gallery(&browser),
        [
            vec![
                "before.png".to_string(),
                format!("t1 · frame {before} · <time>"),
                format!("screenshot {before_path}")
            ],
            vec![
                "after.png".to_string(),
                format!("t1 · frame {after} · <time>"),
                format!("press Enter --wait two --screenshot {after_path}"),
            ],
        ]
    );
    assert_eq!(browser.text("[role=tabpanel] [data-slot=badge]").trim(), "2");
    assert_eq!((thumbnail_width(&browser, 1), thumbnail_width(&browser, 2)), ("200".into(), "200".into()));

    browser.wait_for_status("Live");
    browser.click_button("Take a screenshot");
    wait_for_shots(&browser, 3);
    assert_eq!(
        gallery(&browser)[2],
        [format!("default-t1-{after}.png"), format!("t1 · frame {after} · <time>"), "dashboard".to_string()]
    );
}

#[test]
fn the_lightbox_steps_through_the_screenshots_copies_the_path_and_escape_returns_to_the_thumbnail() {
    let shots = Shots::opened();
    let before = shots.before();
    let after = shots.after();
    let dashboard = Dashboard::start(&shots.at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    open_screenshots(&browser);
    wait_for_shots(&browser, 2);

    the_first_screenshot_opens_in_the_lightbox(&browser, &shots, before);
    the_screenshot_scales_down_to_fit_its_box(&browser);
    next_steps_to_the_second_screenshot(&browser, &shots, after);
    copy_path_copies_it_and_escape_returns_to_the_thumbnail(&browser, &shots);
}

fn the_first_screenshot_opens_in_the_lightbox(browser: &Browser, shots: &Shots, before: u64) {
    open_shot(browser, 1);
    assert_eq!(title(browser), "before.png");
    let before_path = shots.path("before.png");
    assert_eq!(captions(browser), [format!("before.png · t1 · frame {before} · <time> · screenshot {before_path}")]);
    browser.wait_for_text("1 of 2");
    assert_eq!(button_state(browser, "Previous screenshot"), "disabled");
    let image = format!("document.querySelector({})", js_string(&format!("{LIGHTBOX} figure img")));
    browser.wait_until(&format!("{image}?.complete === true && {image}.naturalWidth === 200"));
    assert_eq!(eval_string(browser, &format!("{image}.alt")), format!("before.png, frame {before}"));
}

fn the_screenshot_scales_down_to_fit_its_box(browser: &Browser) {
    assert_eq!(framed_in(browser, "150px", ""), "150 150", "a narrow lightbox scales the screenshot down to its width");
    assert_eq!(
        framed_in(browser, "1000px", "60px"),
        "60 60",
        "a short lightbox scales the screenshot down to its height"
    );
}

fn framed_in(browser: &Browser, width: &str, height: &str) -> String {
    let script = format!(
        "(() => {{ const box = document.querySelector({}); const image = box.querySelector('img'); box.style.width = {}; box.style.height = {}; const frame = box.querySelector('.term-frame').getBoundingClientRect(); box.style.width = ''; box.style.height = ''; const across = {} === '' ? [frame.width, frame.height * image.naturalWidth / image.naturalHeight] : [frame.height, frame.width * image.naturalHeight / image.naturalWidth]; return across.map(Math.round).join(' '); }})()",
        js_string(&format!("{LIGHTBOX} figure .term-box")),
        js_string(width),
        js_string(height),
        js_string(height)
    );
    eval_string(browser, &script)
}

fn next_steps_to_the_second_screenshot(browser: &Browser, shots: &Shots, after: u64) {
    browser.click_button("Next screenshot");
    wait_for_title(browser, "after.png");
    browser.wait_for_text("2 of 2");
    assert_eq!(button_state(browser, "Next screenshot"), "disabled");
    assert_eq!(focused(browser), "Previous screenshot");
    let after_path = shots.path("after.png");
    let by = format!("press Enter --wait two --screenshot {after_path}");
    assert_eq!(captions(browser), [format!("after.png · t1 · frame {after} · <time> · {by}")]);
}

fn copy_path_copies_it_and_escape_returns_to_the_thumbnail(browser: &Browser, shots: &Shots) {
    let after_path = shots.path("after.png");
    browser.click_button("Copy path");
    browser.wait_until(&format!(
        "[...document.querySelectorAll('.toast')].some((toast) => toast.innerText.includes('Copied') && toast.innerText.includes({}))",
        js_string(&after_path)
    ));

    browser.run(&["press", "Escape"]);
    browser.wait_until(&format!("document.querySelector({}) === null", js_string(LIGHTBOX)));
    assert!(focus_is(browser, &format!("{GALLERY} > li:nth-child(1) > button")), "focus is on {}", focused(browser));
}

fn changed_marks() -> String {
    format!(
        "JSON.stringify([...document.querySelectorAll({})].map((figure) => figure.querySelectorAll('[data-testid=changed-cells]').length))",
        js_string(&format!("{LIGHTBOX} figure"))
    )
}

#[test]
fn compare_shows_the_older_screenshot_on_the_left_and_highlights_the_cells_that_changed() {
    let shots = Shots::opened();
    let before = shots.before();
    let after = shots.after();
    shots.at.run(&["screenshot", "--scale", "2", &shots.path("bigger.png")]).success();
    let dashboard = Dashboard::start(&shots.at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    open_screenshots(&browser);
    wait_for_shots(&browser, 3);

    comparing_after_with_before_puts_before_on_the_left(&browser, before, after);
    the_comparison_marks_the_changed_cells_and_cannot_step(&browser);
    back_returns_to_the_screenshot(&browser);
    comparing_screenshots_of_different_sizes_marks_no_cells(&browser);

    browser.run(&["press", "Escape"]);
    browser.wait_until(&format!("document.querySelector({}) === null", js_string(LIGHTBOX)));
    assert!(focus_is(&browser, &format!("{GALLERY} > li:nth-child(2) > button")), "focus is on {}", focused(&browser));
}

fn comparing_after_with_before_puts_before_on_the_left(browser: &Browser, before: u64, after: u64) {
    open_shot(browser, 2);
    browser.click_button("Compare with…");
    browser.wait_until("document.activeElement.tagName === 'SELECT'");
    browser.run(&["select", &format!("{LIGHTBOX} select"), "before.png"]);

    wait_for_title(browser, "after.png compared");
    browser.wait_until(&format!(
        "document.querySelector({})?.innerText === '3 cells changed'",
        js_string(&format!("{LIGHTBOX} [role=status]"))
    ));
    let captions = captions(browser);
    assert_eq!(captions.len(), 2);
    assert!(captions[0].starts_with(&format!("before.png · t1 · frame {before} · ")), "{captions:?}");
    assert!(captions[1].starts_with(&format!("after.png · t1 · frame {after} · ")), "{captions:?}");
}

fn the_comparison_marks_the_changed_cells_and_cannot_step(browser: &Browser) {
    assert_eq!(eval_string(browser, &changed_marks()), "[1,1]");
    assert_eq!(button_state(browser, "Previous screenshot"), "disabled");
    assert_eq!(button_state(browser, "Next screenshot"), "disabled");
    assert_eq!(focused(browser), "Back to screenshot");
}

fn back_returns_to_the_screenshot(browser: &Browser) {
    browser.click_button("Back to screenshot");
    wait_for_title(browser, "after.png");
    assert_eq!(focused(browser), "Compare with…");
}

fn comparing_screenshots_of_different_sizes_marks_no_cells(browser: &Browser) {
    browser.click_button("Compare with…");
    browser.wait_until("document.activeElement.tagName === 'SELECT'");
    browser.run(&["select", &format!("{LIGHTBOX} select"), "bigger.png"]);
    wait_for_title(browser, "after.png compared");
    browser.wait_until(&format!(
        "document.querySelector({})?.innerText === 'Different sizes: no cell comparison'",
        js_string(&format!("{LIGHTBOX} [role=status]"))
    ));
    let captions = captions(browser);
    assert!(captions[0].starts_with("after.png"), "{captions:?}");
    assert!(captions[1].starts_with("bigger.png"), "{captions:?}");
    assert_eq!(eval_string(browser, &changed_marks()), "[0,0]");
}

#[test]
fn a_screenshot_thumbnail_in_activity_opens_the_lightbox_and_escape_returns_to_it() {
    let shots = Shots::opened();
    let dashboard = Dashboard::start(&shots.at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    browser.wait_until(
        "document.querySelectorAll(\"[aria-label='Calls to default t1'] > li:not([aria-busy])\").length === 1",
    );
    shots.before();
    shots.after();
    browser.wait_until(
        "[...document.querySelectorAll(\"[aria-label='Calls to default t1'] button\")].some((thumbnail) => thumbnail.innerText.trim() === 'Open screenshot after.png')",
    );

    browser.run(&["find", "role", "button", "click", "--name", "Open screenshot after.png", "--exact"]);
    wait_for_title(&browser, "after.png");
    browser.wait_for_text("2 of 2");

    browser.run(&["press", "Escape"]);
    browser.wait_until(&format!("document.querySelector({}) === null", js_string(LIGHTBOX)));
    assert_eq!(focused(&browser), "Open screenshot after.png");
}
