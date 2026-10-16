use std::path::Path;

use super::Terminal;
use crate::programs;

#[test]
fn a_step_reports_the_last_command_or_the_first_failure_before_it() {
    let mut unrun = Terminal::default();
    assert_eq!(unrun.report(), "no command ran before this step");
    unrun.act(&["--version"]);
    unrun.setup(&["--version"]);
    assert_eq!(
        (unrun.ran().is_none(), unrun.report()),
        (true, "a step before this one failed: trials() was not given the agent-terminal binary".to_string())
    );
    programs::set(Path::new("/bin/sh"));
    let mut shell = Terminal::default();
    shell.act(&["-c", "printf hi; printf oops >&2"]);
    assert_eq!(shell.report(), "the last command printed \"hi\" and \"oops\"");
    shell.setup(&["-c", "printf no >&2; exit 3"]);
    assert_eq!(
        (shell.ran().is_none(), shell.report()),
        (true, "a step before this one failed: `agent-terminal -c printf no >&2; exit 3` failed: no".to_string())
    );
}
