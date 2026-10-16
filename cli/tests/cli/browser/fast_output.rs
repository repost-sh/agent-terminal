use std::time::Duration;

use super::{Browser, Dashboard};
use crate::support::{AgentTerminal, Redraw, open_redraw};

const STREAMING: Duration = Duration::from_secs(20);
const MIN_PAINTS_PER_SEC: f64 = 30.0;
const MAX_BLOCKED_MS_PER_SEC: f64 = 200.0;
const MAX_FRAMES_BEHIND: u64 = 60;

const MEASURE_FIVE_SECONDS: &str = r#"new Promise((done) => {
  const started = performance.now();
  let paints = 0;
  let blocked = 0;
  const observer = new PerformanceObserver((list) => list.getEntries().forEach((task) => { blocked += task.duration; }));
  observer.observe({ type: "longtask" });
  const paint = () => { paints += 1; if (performance.now() - started < 5000) requestAnimationFrame(paint); else finish(); };
  const finish = () => {
    observer.disconnect();
    const seconds = (performance.now() - started) / 1000;
    const chip = document.getElementById("status-region")?.innerText.match(/frame (\d+)/);
    const gl = document.createElement("canvas").getContext("webgl2");
    const named = gl?.getExtension("WEBGL_debug_renderer_info");
    const gpu = gl ? String(gl.getParameter(named ? named.UNMASKED_RENDERER_WEBGL : gl.RENDERER)) : "no WebGL";
    const renderer = document.querySelector(".xterm-screen canvas") ? "WebGL" : "the DOM";
    done({ paints: paints / seconds, blocked: blocked / seconds, frame: chip ? Number(chip[1]) : 0, renderer, gpu });
  };
  requestAnimationFrame(paint);
})"#;

const PAINTS_ON_A_BLANK_PAGE: &str = r"new Promise((done) => {
  const started = performance.now();
  let paints = 0;
  const paint = () => { paints += 1; if (performance.now() - started < 2000) requestAnimationFrame(paint); else done(paints / ((performance.now() - started) / 1000)); };
  requestAnimationFrame(paint);
})";

#[test]
fn a_tab_that_redraws_sixty_times_a_second_leaves_the_dashboard_responsive_and_on_the_latest_frame() {
    let at = AgentTerminal::new();
    open_redraw(&at, &Redraw { session: "default", hz: "60", repaint: "--partial" });
    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    browser.wait_for_screen("forwarding");

    let streamed = at.frame("default") + 60 * STREAMING.as_secs();
    let limit = (STREAMING * 2).as_millis().to_string();
    at.run(&["--timeout", &limit, "wait", "--frame", &streamed.to_string()]).success();
    let measured: serde_json::Value = browser.eval_json(MEASURE_FIVE_SECONDS);
    let latest = at.frame("default");
    browser.run(&["open", "about:blank"]);
    let blank: f64 = browser.eval_json(PAINTS_ON_A_BLANK_PAGE);

    let paints = measured["paints"].as_f64().expect("paints");
    let blocked = measured["blocked"].as_f64().expect("blocked");
    let shown = measured["frame"].as_u64().expect("frame");
    let renderer = measured["renderer"].as_str().expect("renderer");
    let gpu = measured["gpu"].as_str().expect("gpu");
    assert!(
        paints >= MIN_PAINTS_PER_SEC
            && blocked <= MAX_BLOCKED_MS_PER_SEC
            && latest.saturating_sub(shown) <= MAX_FRAMES_BEHIND,
        "after {STREAMING:?} of a 60 Hz tab the dashboard painted {paints:.1} times a second (at least {MIN_PAINTS_PER_SEC}), \
         its main thread was blocked {blocked:.0} ms a second (at most {MAX_BLOCKED_MS_PER_SEC}), \
         and it showed frame {shown} while the tab was at frame {latest} (at most {MAX_FRAMES_BEHIND} behind); \
         a blank page in the same browser painted {blank:.1} times a second, \
         xterm.js drew with {renderer}, and WebGL ran on {gpu}"
    );
}
