use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::install::Install;
use super::installers::{EVERY_INSTALLER, RELOCATED_BINARY, global_listings};
use crate::support::{AgentTerminal, VERSION};

#[test]
fn upgrade_without_a_detectable_installer_prints_each_installers_command() {
    let at = AgentTerminal::new();
    let binary = Path::new(env!("CARGO_BIN_EXE_agent-terminal")).canonicalize().unwrap();
    let message = format!(
        "cannot tell which installer put agent-terminal at {}, so upgrade cannot update it; run the command of the one you used:\n{EVERY_INSTALLER}",
        binary.display()
    );

    let run = at.run(&["upgrade"]);
    assert_eq!((run.code(), run.stdout().as_str()), (1, ""));
    assert_eq!(run.stderr(), format!("error: {message}\n"));

    let run = at.run(&["--json", "upgrade"]);
    assert_eq!((run.code(), run.stderr().as_str()), (1, ""));
    assert_eq!(run.json(), serde_json::json!({ "error": { "code": 1, "message": message } }));
}

#[test]
fn upgrade_asks_each_installers_global_listing_when_the_path_does_not_tell() {
    let install = Install::new(RELOCATED_BINARY);
    global_listings(&install, "corepack@0.29.4");
    let run = install.upgrade(&["upgrade"]).success();
    assert_eq!(run.stdout(), format!("upgraded agent-terminal {VERSION} to 9.9.9 with yarn\n"));
    assert_eq!(
        install.calls(),
        "npm ls -g --depth=0\npnpm ls -g --depth=0\nyarn global list\nbun pm ls -g\nyarn info @repost/agent-terminal version --silent\nyarn global add @repost/agent-terminal@latest\nyarn global bin\n"
    );
}

#[test]
fn upgrade_cannot_tell_the_installer_when_two_global_listings_have_agent_terminal() {
    let install = Install::new(RELOCATED_BINARY);
    global_listings(&install, "@repost/agent-terminal@0.0.1");
    let run = install.upgrade(&["upgrade"]);
    assert_eq!((run.code(), run.stdout().as_str()), (1, ""));
    assert_eq!(
        run.stderr(),
        format!(
            "error: cannot tell which installer put agent-terminal at {}, so upgrade cannot update it; run the command of the one you used:\n{EVERY_INSTALLER}\n",
            install.binary.canonicalize().unwrap().display()
        )
    );
    assert_eq!(install.calls(), "npm ls -g --depth=0\npnpm ls -g --depth=0\nyarn global list\nbun pm ls -g\n");
}

#[test]
fn upgrade_cannot_tell_the_installer_when_a_global_listing_fails_or_cannot_run() {
    let install = Install::new(RELOCATED_BINARY);
    global_listings(&install, "corepack@0.29.4");
    install.answering(
        "pnpm",
        &[(
            "ls -g --depth=0",
            "echo 'ERR_PNPM_NO_GLOBAL_BIN_DIR  Unable to find the global bin directory' >&2\nexit 1",
        )],
    );
    std::fs::set_permissions(install.bin.join("bun"), std::fs::Permissions::from_mode(0o644)).unwrap();

    let run = install.upgrade(&["upgrade"]);
    assert_eq!((run.code(), run.stdout().as_str()), (1, ""));
    assert_eq!(
        run.stderr(),
        format!(
            "ERR_PNPM_NO_GLOBAL_BIN_DIR  Unable to find the global bin directory\nerror: cannot tell which installer put agent-terminal at {} (`pnpm ls -g --depth=0` failed (exit status: 1); cannot run `bun pm ls -g`: Permission denied (os error 13)), so upgrade cannot update it; run the command of the one you used:\n{EVERY_INSTALLER}\n",
            install.binary.canonicalize().unwrap().display()
        )
    );
    assert_eq!(install.calls(), "npm ls -g --depth=0\npnpm ls -g --depth=0\nyarn global list\n");
}
