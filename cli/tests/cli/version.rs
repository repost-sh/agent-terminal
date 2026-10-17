use crate::support::{AgentTerminal, BUILD, COMMIT, VERSION};

#[test]
fn version_prints_the_version_and_the_commit_it_was_built_from() {
    let at = AgentTerminal::new();
    assert_eq!(at.run(&["version"]).success().stdout(), format!("agent-terminal {VERSION} ({COMMIT})\n"));
    assert_eq!(at.run(&["--version"]).success().stdout(), format!("agent-terminal {VERSION}\n"));
    assert_eq!(
        at.run(&["--json", "version"]).success().json(),
        serde_json::json!({ "version": VERSION, "build": BUILD })
    );
}
