use super::{Browser, Dashboard};
use crate::support::AgentTerminal;

const SCREENSHOT_FONTS_LOADED: &str = r#"["JetBrains Mono NL", "Symbols Nerd Font Mono", "Noto Emoji"].every((family) => Array.from(document.fonts).some((face) => face.status === "loaded" && face.family.replace(/"/g, "") === family))"#;

#[test]
fn the_terminal_draws_with_the_screenshot_fonts() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x4", "--wait", "ready", "--", "sh", "-c", "printf ready; sleep 300"]).success();
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/work/t1"));

    browser.wait_for_screen("ready");
    browser.wait_until(SCREENSHOT_FONTS_LOADED);
}
