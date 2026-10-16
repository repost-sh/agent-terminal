use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use super::{Agent, Daemon};

const STUBBORN: &str =
    "#!/bin/sh\ncase \"$2\" in\n  run) echo ready; exec sleep 60 ;;\n  *) echo 'cannot stop' >&2; exit 1 ;;\nesac\n";

#[test]
fn a_daemon_that_will_not_stop_is_killed_when_it_is_dropped() {
    let work = tempfile::tempdir().unwrap();
    let binary = work.path().join("stubborn");
    std::fs::write(&binary, STUBBORN).unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    let agent = Agent { side: "change", binary, work: work.path().to_path_buf(), home: "home".into() };
    let daemon = Daemon::start(agent).unwrap();
    let pid = daemon.process.id().to_string();
    drop(daemon);
    let state = Command::new("ps").args(["-o", "stat=", "-p", &pid]).output().unwrap();
    assert!(String::from_utf8_lossy(&state.stdout).trim_start().starts_with('Z'), "{state:?}");
}

const UNREADY: &str = "#!/bin/sh\ncase \"$2\" in\n  run) echo $$ > pid; echo 'warming up'; exec sleep 60 ;;\n  *) touch stopped; exit 1 ;;\nesac\n";

#[test]
fn a_program_that_never_says_ready_is_killed_without_being_asked_to_stop() {
    let work = tempfile::tempdir().unwrap();
    let binary = work.path().join("unready");
    std::fs::write(&binary, UNREADY).unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    let agent = Agent { side: "change", binary, work: work.path().to_path_buf(), home: "home".into() };
    let refused = Daemon::start(agent).err().map(|error| error.to_string());
    let pid = std::fs::read_to_string(work.path().join("pid")).unwrap();
    let running = Command::new("ps").args(["-o", "stat=", "-p", pid.trim()]).output().unwrap();
    assert_eq!(
        (refused, work.path().join("stopped").exists(), String::from_utf8_lossy(&running.stdout).trim().to_string()),
        (Some(r#"the change daemon did not start: "warming up\n""#.to_string()), false, String::new())
    );
}
