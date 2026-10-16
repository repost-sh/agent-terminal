use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{Agent, Binary, Daemon, Dialect};

fn executable(work: &Path, name: &str, script: &str) -> PathBuf {
    let path = work.join(name);
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn agent(side: &'static str, work: &Path, script: &str, dialect: Dialect) -> Agent {
    let binary = Binary { path: executable(work, "agent-terminal", script), dialect };
    Agent { side, binary, work: work.to_path_buf(), home: "home".into() }
}

const STUBBORN: &str =
    "#!/bin/sh\ncase \"$2\" in\n  run) echo ready; exec sleep 60 ;;\n  *) echo 'cannot stop' >&2; exit 1 ;;\nesac\n";

#[test]
fn a_daemon_that_will_not_stop_is_killed_when_it_is_dropped() {
    let work = tempfile::tempdir().unwrap();
    let daemon = Daemon::start(agent("change", work.path(), STUBBORN, Dialect::Named)).unwrap();
    let pid = daemon.process.id().to_string();
    drop(daemon);
    let state = Command::new("ps").args(["-o", "stat=", "-p", &pid]).output().unwrap();
    assert!(String::from_utf8_lossy(&state.stdout).trim_start().starts_with('Z'), "{state:?}");
}

const UNREADY: &str = "#!/bin/sh\ncase \"$2\" in\n  run) echo $$ > pid; echo 'warming up'; exec sleep 60 ;;\n  *) touch stopped; exit 1 ;;\nesac\n";

#[test]
fn a_program_that_never_says_ready_is_killed_without_being_asked_to_stop() {
    let work = tempfile::tempdir().unwrap();
    let refused =
        Daemon::start(agent("change", work.path(), UNREADY, Dialect::Named)).err().map(|error| error.to_string());
    let pid = std::fs::read_to_string(work.path().join("pid")).unwrap();
    let running = Command::new("ps").args(["-o", "stat=", "-p", pid.trim()]).output().unwrap();
    assert_eq!(
        (refused, work.path().join("stopped").exists(), String::from_utf8_lossy(&running.stdout).trim().to_string()),
        (Some(r#"the change daemon did not start: "warming up\n""#.to_string()), false, String::new())
    );
}

const EXPLAINS_SESSION_NEW: &str = "#!/bin/sh\n[ \"$*\" = 'session new --help' ]\n";

#[test]
fn a_build_that_explains_session_new_speaks_the_named_dialect_and_any_other_the_unnamed() {
    let work = tempfile::tempdir().unwrap();
    let explains = executable(work.path(), "explains", EXPLAINS_SESSION_NEW);
    let refuses = executable(work.path(), "refuses", "#!/bin/sh\nexit 1\n");
    let dialects = [explains, refuses, work.path().join("missing")].map(|path| Binary::detect(path).dialect);
    assert_eq!(dialects, [Dialect::Named, Dialect::Unnamed, Dialect::Unnamed]);
}

const RECORDER: &str = "#!/bin/sh\necho \"$*\" >> calls\n";

fn session_and_tab(dialect: Dialect) -> (Result<String, String>, String) {
    let work = tempfile::tempdir().unwrap();
    let agent = agent("base", work.path(), RECORDER, dialect);
    let opened = agent.new_session("perf").and_then(|()| agent.open("screen", &["program"]));
    let calls = std::fs::read_to_string(work.path().join("calls")).unwrap();
    (opened.map_err(|error| error.to_string()), calls)
}

#[test]
fn a_named_build_makes_the_session_and_opens_the_tab_by_its_name() {
    assert_eq!(
        session_and_tab(Dialect::Named),
        (Ok("screen".to_string()), "session new perf\nopen --name screen program\n".to_string())
    );
}

#[test]
fn an_unnamed_build_skips_session_new_and_its_tab_is_the_first_one() {
    assert_eq!(session_and_tab(Dialect::Unnamed), (Ok("t1".to_string()), "open program\n".to_string()));
}

const REFUSER: &str = "#!/bin/sh\necho \"error: $* is refused\" >&2\necho 'see the log'\nexit 1\n";

#[test]
fn a_refused_command_names_itself_the_side_and_all_the_build_printed() {
    let work = tempfile::tempdir().unwrap();
    let named = agent("base", work.path(), REFUSER, Dialect::Named);
    let unnamed = agent("change", work.path(), REFUSER, Dialect::Unnamed);
    let refusals = [
        named.new_session("taken").map(|()| String::new()),
        named.open("taken", &["program"]),
        unnamed.open("taken", &["program"]),
    ]
    .map(|refused| refused.map_err(|error| error.to_string()));
    assert_eq!(
        refusals,
        [
            Err("`agent-terminal session new taken` failed for the base (exit status: 1): error: session new taken is refused\nsee the log\n"
                .to_string()),
            Err("`agent-terminal open --name taken program` failed for the base (exit status: 1): error: open --name taken program is refused\nsee the log\n"
                .to_string()),
            Err("`agent-terminal open program` failed for the change (exit status: 1): error: open program is refused\nsee the log\n"
                .to_string()),
        ]
    );
}
