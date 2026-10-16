use super::shipped::shipped;
use super::stub::{put_stub, stub_in, stub_of_version};
use crate::support::{AgentTerminal, VERSION};

#[test]
fn install_skills_writes_the_discovery_stub_into_the_agent_skills_folder() {
    let at = AgentTerminal::new();
    let user = at.home().join("user");
    let run = at.run_with_env(&["install", "--skills"], &[("HOME", &user)]).success();
    let stub = stub_in(&user);
    assert_eq!(run.stdout(), format!("installed the agent-terminal skill {VERSION} at {}\n", stub.display()));
    let installed = std::fs::read_to_string(&stub).unwrap();
    assert_eq!(installed, at.run(&["skills", "get", "agent-terminal"]).success().stdout());
    let shipped = shipped("skills/agent-terminal/SKILL.md");
    let body = shipped.split_once("\n---\n").expect("frontmatter").1;
    assert!(installed.starts_with("---\nname: agent-terminal\n"), "{installed}");
    assert!(installed.contains("\nInstall: `npm i -g @repost/agent-terminal`\n"), "{installed}");
    assert!(
        installed.ends_with(&format!(
            "\nallowed-tools: Bash(agent-terminal:*), Bash(npx @repost/agent-terminal:*)\nmetadata:\n  version: \
             \"{VERSION}\"\n---\n{body}"
        )),
        "{installed}"
    );
}

pub(super) fn installed_stub(at: &AgentTerminal) -> String {
    let user = at.home().join("user");
    at.run_with_env(&["install", "--skills"], &[("HOME", &user)]).success();
    std::fs::read_to_string(stub_in(&user)).unwrap()
}

#[test]
fn install_without_flags_installs_the_stub_too() {
    let at = AgentTerminal::new();
    let user = at.home().join("user");
    at.run_with_env(&["install"], &[("HOME", &user)]).success();
    let installed = std::fs::read_to_string(stub_in(&user)).unwrap();
    assert_eq!(installed, at.run(&["skills", "get", "agent-terminal"]).success().stdout());
}

#[test]
fn install_skills_keeps_a_newer_stub() {
    let at = AgentTerminal::new();
    let user = at.home().join("user");
    put_stub(&user, &stub_of_version("999.0.0"));
    let run = at.run_with_env(&["install", "--skills"], &[("HOME", &user)]).success();
    let stub = stub_in(&user);
    assert_eq!(
        run.stdout(),
        format!("kept the newer agent-terminal skill 999.0.0 at {} (this is {VERSION})\n", stub.display())
    );
    assert_eq!(std::fs::read_to_string(stub).unwrap(), stub_of_version("999.0.0"));
}

#[test]
fn install_skills_replaces_an_older_or_unversioned_stub() {
    let at = AgentTerminal::new();
    let user = at.home().join("user");
    let current = at.run(&["skills", "get", "agent-terminal"]).success().stdout();
    for existing in [stub_of_version("0.0.1"), stub_of_version("0.1.0-alpha.1"), "# my notes\n".to_string()] {
        put_stub(&user, &existing);
        let run = at.run_with_env(&["install", "--skills"], &[("HOME", &user)]).success();
        assert_eq!(
            run.stdout(),
            format!("installed the agent-terminal skill {VERSION} at {}\n", stub_in(&user).display())
        );
        assert_eq!(std::fs::read_to_string(stub_in(&user)).unwrap(), current, "replacing:\n{existing}");
    }
}

#[test]
fn install_skills_as_json() {
    let at = AgentTerminal::new();
    let user = at.home().join("user");
    let stub = stub_in(&user);
    let installed = at.run_with_env(&["--json", "install", "--skills"], &[("HOME", &user)]).success().json();
    assert_eq!(installed, serde_json::json!({ "installed": true, "path": stub, "version": VERSION }));
    put_stub(&user, &stub_of_version("999.0.0"));
    let kept = at.run_with_env(&["--json", "install", "--skills"], &[("HOME", &user)]).success().json();
    assert_eq!(kept, serde_json::json!({ "installed": false, "path": stub, "version": "999.0.0" }));
}

#[test]
fn simultaneous_installs_leave_one_complete_stub() {
    let at = AgentTerminal::new();
    let user = at.home().join("user");
    std::thread::scope(|scope| {
        let installs: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| at.run_with_env(&["install", "--skills"], &[("HOME", &user)]).code()))
            .collect();
        let codes: Vec<i32> = installs.into_iter().map(|install| install.join().unwrap()).collect();
        assert_eq!(codes, vec![0; 8]);
    });
    let installed = std::fs::read_to_string(stub_in(&user)).unwrap();
    assert_eq!(installed, at.run(&["skills", "get", "agent-terminal"]).success().stdout());
}
