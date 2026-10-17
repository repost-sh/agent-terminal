use crate::support::{AgentTerminal, fixture};

fn open_echo(at: &AgentTerminal, modes: &[&str]) {
    let echo = fixture("input-echo");
    let mut args = vec!["open", "--name", "app", "--size", "60x4", "--wait", "ready", echo.as_str()];
    args.extend_from_slice(modes);
    at.run(&args).success();
}

#[test]
fn focus_in_and_out_are_reported_to_an_app_that_asked_for_them() {
    let at = AgentTerminal::new();
    open_echo(&at, &["--focus-events"]);
    assert_eq!(at.run(&["focus", "out"]).success().stdout(), "reported focus out to work app\n");
    let reported = at.run(&["focus", "in", "--wait", r"\e[O\e[I"]).success();
    assert_eq!(reported.stdout(), "reported focus in to work app\n");
    assert_eq!(at.received("work"), r"\e[O\e[I");
}

#[test]
fn focus_is_not_sent_to_an_app_that_did_not_ask_for_reports() {
    let at = AgentTerminal::new();
    open_echo(&at, &[]);
    assert_eq!(
        at.run(&["focus", "out"]).success().stdout(),
        "reported focus out to work app (not sent: the app does not ask for focus reports)\n"
    );
    let reported = at.run(&["--json", "focus", "in"]).success().json();
    assert_eq!(
        reported,
        serde_json::json!({ "session": "work", "tab": "app", "input": "focus", "focused": true, "bytes": 0 })
    );
    at.run(&["press", "x", "--wait", "x"]).success();
    assert_eq!(at.received("work"), "x");
}

#[test]
fn focus_goes_in_or_out() {
    let at = AgentTerminal::new();
    let refused = at.run(&["focus", "sideways"]);
    assert_eq!(refused.code(), 1, "{}", refused.stderr());
    assert!(refused.stderr().contains("focus goes in or out, not sideways"), "{}", refused.stderr());
}
