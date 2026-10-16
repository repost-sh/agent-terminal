use std::path::{Path, PathBuf};

pub(crate) fn stub_in(user: &Path) -> PathBuf {
    user.join(".claude/skills/agent-terminal/SKILL.md")
}

pub(crate) fn stub_of_version(version: &str) -> String {
    format!(
        "---\nname: agent-terminal\ndescription: The stub of agent-terminal {version}\nmetadata:\n  version: \"{version}\"\n---\n\n# agent-terminal {version}\n"
    )
}

pub(crate) fn put_stub(user: &Path, contents: &str) {
    let stub = stub_in(user);
    std::fs::create_dir_all(stub.parent().unwrap()).unwrap();
    std::fs::write(stub, contents).unwrap();
}
