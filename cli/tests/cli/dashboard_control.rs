use serde_json::json;

use crate::support::{AgentTerminal, DashboardSocket, dashboard, fixture};

const CONTROL: &str = "/api/session/default/t1/control";

#[test]
fn a_human_holding_control_in_the_dashboard_types_and_sees_each_refused_agent_command_as_typed() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "40x4", "--wait", "ready", &fixture("input-echo")]).success();
    let mut control = DashboardSocket::open(dashboard(&at), CONTROL);
    assert_eq!(control.next(), json!({ "type": "held" }));

    control.send(&json!({ "typed": "hi" }));
    at.run(&["wait", "hi"]).success();
    let refused = at.run(&["--json", "--session", "default", "type", "--", "-x"]);
    assert_eq!(refused.code(), 4, "{}", refused.stderr());
    assert_eq!(control.next(), json!({ "type": "agent-refused", "command": "type -- -x" }));
    assert_eq!(at.received("default"), "hi");
}
