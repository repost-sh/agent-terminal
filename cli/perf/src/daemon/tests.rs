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
