use serde_json::json;

use crate::support::{AgentTerminal, DashboardSocket, dashboard, fixture};

const CONTROL: &str = "/api/session/work/app/control";

#[test]
fn a_human_holding_control_in_the_dashboard_types_and_sees_each_refused_agent_command_as_typed() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "40x4", "--wait", "ready", &fixture("input-echo")]).success();
    let mut control = DashboardSocket::open(dashboard(&at), CONTROL);
    assert_eq!(control.next(), json!({ "type": "held" }));

    control.send(&json!({ "typed": "hi" }));
    at.run(&["wait", "hi"]).success();
    let refused = at.run(&["--json", "--session", "work", "type", "--", "-x"]);
    assert_eq!(refused.code(), 4, "{}", refused.stderr());
    assert_eq!(control.next(), json!({ "type": "agent-refused", "command": "type -- -x" }));
    assert_eq!(at.received("work"), "hi");
}

#[test]
fn a_human_holding_control_in_the_dashboard_sends_the_mouse_and_focus_and_only_presses_releases_and_wheels_are_logged()
{
    let at = AgentTerminal::new();
    let echo = fixture("input-echo");
    let modes = ["--mouse", "motion", "--mouse-encoding", "sgr", "--focus-events"];
    let mut args = vec!["open", "--name", "app", "--size", "80x4", "--wait", "ready", echo.as_str()];
    args.extend_from_slice(&modes);
    at.run(&args).success();
    let mut control = DashboardSocket::open(dashboard(&at), CONTROL);
    assert_eq!(control.next(), json!({ "type": "held" }));

    control
        .send(&json!({ "mouse": { "event": "down", "row": 2, "col": 3, "button": "left", "mods": { "ctrl": true } } }));
    control.send(&json!({ "mouse": { "event": "move", "row": 2, "col": 4 } }));
    control.send(&json!({ "mouse": { "event": "up", "row": 2, "col": 4, "button": "left" } }));
    control.send(&json!({ "mouse": { "event": "wheel", "row": 2, "col": 4, "direction": "down" } }));
    control.send(&json!({ "focused": false }));
    control.send(&json!({ "focused": true }));
    let expected = r"\e[<16;3;2M\e[<32;4;2M\e[<0;4;2m\e[<65;4;2M\e[O\e[I";
    at.run(&["wait", expected]).success();
    assert_eq!(at.received("work"), expected);

    let logged: Vec<_> = at.activity("work").into_iter().filter(|entry| entry.get("human").is_some()).collect();
    assert_eq!(
        logged.iter().map(|entry| entry["command"].clone()).collect::<Vec<_>>(),
        [json!("mouse down '2,3' --ctrl"), json!("mouse up '2,4'"), json!("mouse wheel down '2,4'")]
    );
    assert_eq!(
        logged[0]["human"],
        json!({ "mouse": { "event": "down", "row": 2, "col": 3, "button": "left", "mods": { "shift": false, "alt": false, "ctrl": true } } })
    );
}

#[test]
fn a_mouse_event_from_the_dashboard_is_refused_when_the_app_has_no_mouse_support() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "40x4", "--wait", "ready", &fixture("input-echo")]).success();
    let mut control = DashboardSocket::open(dashboard(&at), CONTROL);
    assert_eq!(control.next(), json!({ "type": "held" }));
    control.send(&json!({ "mouse": { "event": "down", "row": 2, "col": 3, "button": "left" } }));
    let refused = control.next();
    assert_eq!(refused["type"], json!("refused"));
    assert!(refused["message"].as_str().unwrap_or("").contains("this app has no mouse support"), "{refused}");
    control.send(&json!({ "typed": "k" }));
    at.run(&["wait", "k"]).success();
    assert_eq!(at.received("work"), "k");
}
