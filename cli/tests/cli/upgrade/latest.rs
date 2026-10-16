use super::install::Install;
use super::installers::{INSTALLS, Installed, NPM_INSTALLED_BINARY};
use crate::support::VERSION;

#[test]
fn upgrade_asks_the_installer_for_the_latest_version_and_reinstalls_with_it() {
    for installed in INSTALLS {
        upgrade_reinstalls_with_the_installer(installed);
        upgrade_in_json_reinstalls_with_the_installer(installed);
    }
}

fn upgrade_reinstalls_with_the_installer(installed: &Installed) {
    let install = Install::new(installed.binary);
    install.answering(installed.manager, installed.answers);
    let run = install.upgrade(&["upgrade"]).success();
    assert_eq!(install.calls(), installed.calls);
    assert_eq!(run.stdout(), format!("upgraded agent-terminal {VERSION} to 9.9.9 with {}\n", installed.installer));
    assert!(run.stderr().contains("changed 1 package"), "the installer's output goes to stderr: {}", run.stderr());
}

fn upgrade_in_json_reinstalls_with_the_installer(installed: &Installed) {
    let install = Install::new(installed.binary);
    install.answering(installed.manager, installed.answers);
    let upgraded = install.upgrade(&["--json", "upgrade"]).success().json();
    assert_eq!(install.calls(), installed.calls);
    assert_eq!(
        upgraded,
        serde_json::json!({
            "installed_with": installed.installer,
            "from": VERSION,
            "latest": "9.9.9",
            "to": "9.9.9",
        })
    );
}

#[test]
fn upgrade_does_not_reinstall_when_the_running_version_is_the_latest() {
    let install = Install::new(NPM_INSTALLED_BINARY);
    install.answering("npm", &[("view @repost/agent-terminal version", &format!("echo {VERSION}"))]);

    let run = install.upgrade(&["upgrade"]).success();
    assert_eq!(run.stdout(), format!("agent-terminal {VERSION} is already the latest version\n"));
    let latest = install.upgrade(&["--json", "upgrade"]).success().json();
    assert_eq!(
        latest,
        serde_json::json!({
            "installed_with": "npm",
            "from": VERSION,
            "latest": VERSION,
            "to": VERSION,
        })
    );
    assert_eq!(install.calls(), "npm view @repost/agent-terminal version\n".repeat(2));
}

#[test]
fn upgrade_says_so_when_the_registry_cannot_be_reached() {
    let install = Install::new(NPM_INSTALLED_BINARY);
    install.answering(
        "npm",
        &[(
            "view @repost/agent-terminal version",
            "echo 'npm error code ENOTFOUND' >&2\necho 'npm error network request to https://registry.npmjs.org/@repost%2fagent-terminal failed' >&2\nexit 1",
        )],
    );
    let npm_said = "npm error code ENOTFOUND\nnpm error network request to https://registry.npmjs.org/@repost%2fagent-terminal failed\n";
    let message = "cannot reach the registry to find the latest version: `npm view @repost/agent-terminal version` failed (exit status: 1); see its output above";

    let run = install.upgrade(&["upgrade"]);
    assert_eq!((run.code(), run.stdout().as_str()), (1, ""));
    assert_eq!(run.stderr(), format!("{npm_said}error: {message}\n"));
    the_json_error_says_the_registry_cannot_be_reached(&install, npm_said, message);
    assert_eq!(install.calls(), "npm view @repost/agent-terminal version\n".repeat(2), "nothing is reinstalled");
}

fn the_json_error_says_the_registry_cannot_be_reached(install: &Install, npm_said: &str, message: &str) {
    let run = install.upgrade(&["--json", "upgrade"]);
    assert_eq!(run.code(), 1);
    assert_eq!(run.json(), serde_json::json!({ "error": { "code": 1, "message": message } }));
    assert_eq!(run.stderr(), npm_said);
}

#[test]
fn upgrade_says_so_when_the_homebrew_tap_cannot_be_reached() {
    let install = Install::new("homebrew/Cellar/agent-terminal/0.0.1/bin/agent-terminal");
    install.answering(
        "brew",
        &[("update", "echo 'fatal: unable to access https://github.com/repost-sh/homebrew-tap/' >&2\nexit 1")],
    );
    let run = install.upgrade(&["upgrade"]);
    assert_eq!((run.code(), run.stdout().as_str()), (1, ""));
    assert_eq!(
        run.stderr(),
        "fatal: unable to access https://github.com/repost-sh/homebrew-tap/\nerror: cannot reach the Homebrew tap to find the latest version: `brew update` failed (exit status: 1); see its output above\n"
    );
    assert_eq!(install.calls(), "brew update\n");
}

#[test]
fn upgrade_fails_when_the_package_manager_fails_or_is_missing() {
    let install = Install::new(NPM_INSTALLED_BINARY);

    let run = install.upgrade(&["upgrade"]);
    assert_eq!((run.code(), run.stdout().as_str()), (1, ""));
    assert!(
        run.stderr().starts_with("error: cannot run `npm view @repost/agent-terminal version`: "),
        "{}",
        run.stderr()
    );

    install.answering(
        "npm",
        &[
            ("view @repost/agent-terminal version", "echo 9.9.9"),
            ("install -g @repost/agent-terminal@latest", "echo 'npm error 404 Not Found' >&2\nexit 1"),
        ],
    );
    let run = install.upgrade(&["upgrade"]);
    assert_eq!((run.code(), run.stdout().as_str()), (1, ""));
    assert_eq!(
        run.stderr(),
        "npm error 404 Not Found\nerror: `npm install -g @repost/agent-terminal@latest` failed (exit status: 1); see its output above\n"
    );
}
