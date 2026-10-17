use std::path::PathBuf;

use super::install::Install;
use super::installers::{NEWER_ON_NPM, NPM_INSTALLED_BINARY};
use crate::support::VERSION;

const OLDER_STUB: &str = "---\nname: agent-terminal\ndescription: The stub of agent-terminal 0.0.1\nmetadata:\n  version: \"0.0.1\"\n---\n\n# agent-terminal 0.0.1\n";

fn older_stubs_in_every_skills_folder(install: &Install) -> Vec<PathBuf> {
    let stubs: Vec<PathBuf> = [".claude/skills", ".agents/skills"]
        .iter()
        .map(|folder| install.user.join(folder).join("agent-terminal/SKILL.md"))
        .collect();
    for stub in &stubs {
        std::fs::create_dir_all(stub.parent().unwrap()).unwrap();
        std::fs::write(stub, OLDER_STUB).unwrap();
    }
    stubs
}

#[test]
fn upgrade_leaves_an_installed_skill_stub_byte_for_byte() {
    let install = Install::new(NPM_INSTALLED_BINARY);
    install.answering("npm", NEWER_ON_NPM);
    let stubs = older_stubs_in_every_skills_folder(&install);

    let run = install.upgrade(&["upgrade"]).success();
    assert_eq!(run.stdout(), format!("upgraded agent-terminal {VERSION} to 9.9.9 with npm\n"));
    let upgraded = install.upgrade(&["--json", "upgrade"]).success().json();
    assert_eq!(
        upgraded,
        serde_json::json!({ "installed_with": "npm", "from": VERSION, "latest": "9.9.9", "to": "9.9.9" })
    );
    for stub in &stubs {
        assert_eq!(std::fs::read(stub).unwrap(), OLDER_STUB.as_bytes(), "upgrade rewrote {}", stub.display());
    }
}
