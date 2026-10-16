use crate::support::{AgentTerminal, VERSION};

#[test]
fn version_prints_the_version() {
    let at = AgentTerminal::new();
    let expected = format!("agent-terminal {VERSION}\n");
    assert_eq!(at.run(&["version"]).success().stdout(), expected);
    assert_eq!(at.run(&["--version"]).success().stdout(), expected);
    assert_eq!(at.run(&["--json", "version"]).success().json(), serde_json::json!({ "version": VERSION }));
}
