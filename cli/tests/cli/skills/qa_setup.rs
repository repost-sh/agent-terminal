use std::path::Path;

use super::guide_commands::code_in;
use super::shipped::shipped;
use crate::support::AgentTerminal;

fn the_setup_copies_the_report_template(skill: &str) {
    let at = AgentTerminal::new();
    let guide = at.run(&["skills", "get", skill]).success().stdout();
    let copy = code_in(&guide)
        .into_iter()
        .find(|line| line.contains("templates/report.md"))
        .unwrap_or_else(|| panic!("the {skill} guide copies its report template"));
    let work = at.home().join("work");
    std::fs::create_dir_all(work.join(format!("{skill}-output"))).unwrap();
    let bin = Path::new(env!("CARGO_BIN_EXE_agent-terminal")).parent().unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let run = std::process::Command::new("sh")
        .args(["-c", copy])
        .current_dir(&work)
        .env("PATH", path)
        .env("AGENT_TERMINAL_HOME", at.home())
        .output()
        .unwrap();
    assert!(run.status.success(), "`{copy}` failed:\n{}", String::from_utf8_lossy(&run.stderr));
    assert_eq!(
        std::fs::read_to_string(work.join(format!("{skill}-output/report.md"))).unwrap(),
        shipped(&format!("skill-data/{skill}/templates/report.md"))
    );
}

#[test]
fn the_tui_qa_setup_copies_the_report_template() {
    the_setup_copies_the_report_template("tui-qa");
}

#[test]
fn the_cli_qa_setup_copies_the_report_template() {
    the_setup_copies_the_report_template("cli-qa");
}
