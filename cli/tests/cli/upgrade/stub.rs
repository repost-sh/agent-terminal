use super::install::Install;
use super::installers::{NEWER_ON_NPM, NPM_INSTALLED_BINARY, REINSTALL};
use crate::skills::{put_stub, shipped, stub_of_version};
use crate::support::VERSION;

#[test]
fn upgrade_refreshes_an_installed_skill_stub() {
    let install = Install::new(NPM_INSTALLED_BINARY);
    install.answering("npm", NEWER_ON_NPM);
    put_stub(&install.user, &stub_of_version("0.0.1"));
    let run = install.upgrade(&["upgrade"]).success();
    assert_eq!(
        run.stdout(),
        format!(
            "upgraded agent-terminal {VERSION} to 9.9.9 with npm\nrefreshed the agent-terminal skill {VERSION} at {}\n",
            install.stub().display()
        )
    );
    let refreshed = shipped("skills/agent-terminal/SKILL.md");
    assert_eq!(std::fs::read_to_string(install.stub()).unwrap(), refreshed);

    let install = Install::new(NPM_INSTALLED_BINARY);
    install.answering("npm", NEWER_ON_NPM);
    put_stub(&install.user, &stub_of_version("0.0.1"));
    let upgraded = install.upgrade(&["--json", "upgrade"]).success().json();
    assert_eq!(
        upgraded,
        serde_json::json!({
            "installed_with": "npm",
            "from": VERSION,
            "latest": "9.9.9",
            "to": "9.9.9",
            "stub_refreshed": true,
        })
    );
    assert_eq!(std::fs::read_to_string(install.stub()).unwrap(), refreshed);
}

fn npm_install_whose_new_version_cannot_write_the_stub() -> Install {
    let install = Install::new(NPM_INSTALLED_BINARY);
    install.answering("npm", NEWER_ON_NPM);
    install.published_binary(
        "if [ \"$1\" = --version ]; then echo agent-terminal 9.9.9; else echo '{\"error\":{\"code\":1,\"message\":\"write the stub: No space left on device\"}}'; exit 1; fi\n",
    );
    put_stub(&install.user, &stub_of_version("0.0.1"));
    install
}

#[test]
fn upgrade_says_when_the_new_version_cannot_refresh_the_skill_stub() {
    let install = npm_install_whose_new_version_cannot_write_the_stub();
    let run = install.upgrade(&["upgrade"]).success();
    assert_eq!(
        run.stdout(),
        format!(
            "upgraded agent-terminal {VERSION} to 9.9.9 with npm\ncannot refresh the agent-terminal skill (write the stub: No space left on device); run `agent-terminal install --skills`\n"
        )
    );

    let install = npm_install_whose_new_version_cannot_write_the_stub();
    let upgraded = install.upgrade(&["--json", "upgrade"]).success().json();
    assert_eq!(
        upgraded,
        serde_json::json!({
            "installed_with": "npm",
            "from": VERSION,
            "latest": "9.9.9",
            "to": "9.9.9",
            "stub_refreshed": false,
            "stub_error": "write the stub: No space left on device",
        })
    );
    assert_eq!(std::fs::read_to_string(install.stub()).unwrap(), stub_of_version("0.0.1"));
}

fn npm_install_whose_global_folder_cannot_be_found() -> Install {
    let install = Install::new(NPM_INSTALLED_BINARY);
    install.answering(
        "npm",
        &[
            ("view @repost/agent-terminal version", "echo 9.9.9"),
            ("install -g @repost/agent-terminal@latest", REINSTALL),
            ("prefix -g", "echo 'npm error code EACCES' >&2\nexit 1"),
        ],
    );
    put_stub(&install.user, &stub_of_version("0.0.1"));
    install
}

#[test]
fn upgrade_says_when_it_cannot_find_the_new_version_through_the_installer() {
    let reason = "`npm prefix -g` failed (exit status: 1); see its output above";

    let install = npm_install_whose_global_folder_cannot_be_found();
    let run = install.upgrade(&["upgrade"]).success();
    assert_eq!(
        run.stdout(),
        format!(
            "upgraded agent-terminal {VERSION} with npm, but cannot tell the version now installed ({reason}); run `agent-terminal version`\ncannot refresh the agent-terminal skill ({reason}); run `agent-terminal install --skills`\n"
        )
    );
    assert_eq!(run.stderr(), "changed 1 package\nnpm error code EACCES\n");

    let install = npm_install_whose_global_folder_cannot_be_found();
    let upgraded = install.upgrade(&["--json", "upgrade"]).success().json();
    assert_eq!(
        upgraded,
        serde_json::json!({
            "installed_with": "npm",
            "from": VERSION,
            "latest": "9.9.9",
            "to": null,
            "version_unknown": reason,
            "stub_refreshed": false,
            "stub_error": reason,
        })
    );
    assert_eq!(
        install.calls(),
        "npm view @repost/agent-terminal version\nnpm install -g @repost/agent-terminal@latest\nnpm prefix -g\n"
    );
    assert_eq!(std::fs::read_to_string(install.stub()).unwrap(), stub_of_version("0.0.1"));
}
