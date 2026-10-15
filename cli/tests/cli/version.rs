use std::path::{Path, PathBuf};
use std::process::Command;

use crate::skills::{put_stub, stub_in, stub_of_version};
use crate::support::{AgentTerminal, Run};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[test]
fn version_prints_the_version() {
    let at = AgentTerminal::new();
    let expected = format!("agent-terminal {VERSION}\n");
    assert_eq!(at.run(&["version"]).success().stdout(), expected);
    assert_eq!(at.run(&["--version"]).success().stdout(), expected);
    assert_eq!(at.run(&["--json", "version"]).success().json(), serde_json::json!({ "version": VERSION }));
}

struct Install {
    at: AgentTerminal,
    binary: PathBuf,
    new_binary: PathBuf,
    bin: PathBuf,
    log: PathBuf,
    user: PathBuf,
}

impl Install {
    fn new(binary: &str) -> Self {
        let at = AgentTerminal::new();
        let binary = at.home().join(binary);
        std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
        written_in_a_child(Command::new("cp").arg(env!("CARGO_BIN_EXE_agent-terminal")).arg(&binary));
        let bin = at.home().join("fake-bin");
        std::fs::create_dir_all(&bin).unwrap();
        let install = Self {
            new_binary: at.home().join("agent-terminal-9.9.9"),
            log: at.home().join("package-managers.log"),
            user: at.home().join("user"),
            binary,
            bin,
            at,
        };
        install.published_binary(&format!(
            "if [ \"$1\" = --version ]; then echo agent-terminal 9.9.9; else exec '{}' \"$@\"; fi\n",
            env!("CARGO_BIN_EXE_agent-terminal")
        ));
        install
    }

    fn published_binary(&self, script: &str) {
        executable(&self.new_binary, &format!("#!/bin/sh\n{script}"));
    }

    fn package_manager(&self, name: &str, script: &str) {
        executable(&self.bin.join(name), &format!("#!/bin/sh\necho \"{name} $*\" >> \"$CALLS\"\n{script}"));
    }

    fn answering(&self, name: &str, answers: &[(&str, &str)]) {
        let arms: String = answers.iter().map(|(args, script)| format!("'{args}')\n{script}\n;;\n")).collect();
        self.package_manager(name, &format!("case \"$*\" in\n{arms}*)\nexit 64\n;;\nesac\n"));
    }

    fn upgrade(&self, args: &[&str]) -> Run {
        let path = format!("{}:/usr/bin:/bin", self.bin.display());
        let env = [
            ("PATH", path.as_str()),
            ("CALLS", &*self.log.to_string_lossy()),
            ("BINARY", &*self.binary.to_string_lossy()),
            ("NEW_BINARY", &*self.new_binary.to_string_lossy()),
            ("HOME", &*self.user.to_string_lossy()),
        ];
        self.at.run_program(&self.binary, args, "", &env)
    }

    fn calls(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    fn stub(&self) -> PathBuf {
        stub_in(&self.user)
    }
}

fn executable(path: &Path, contents: &str) {
    written_in_a_child(
        Command::new("sh").args(["-c", "printf '%s' \"$1\" > \"$2\" && chmod 755 \"$2\"", "sh", contents]).arg(path),
    );
}

fn written_in_a_child(command: &mut Command) {
    let status = command.status().expect("run the command that writes the executable");
    assert!(status.success(), "{command:?} failed with {status}");
}

const NPM_INSTALLED_BINARY: &str =
    "prefix/lib/node_modules/@repost/agent-terminal/node_modules/@repost/agent-terminal-linux-x64/bin/agent-terminal";

const REINSTALL: &str = "cp \"$NEW_BINARY\" \"$BINARY.new\"\nmv \"$BINARY.new\" \"$BINARY\"\necho 'changed 1 package'";

const NEWER_ON_NPM: &[(&str, &str)] =
    &[("view @repost/agent-terminal version", "echo 9.9.9"), ("install -g @repost/agent-terminal@latest", REINSTALL)];

const NEWER_ON_PNPM: &[(&str, &str)] =
    &[("view @repost/agent-terminal version", "echo 9.9.9"), ("add -g @repost/agent-terminal@latest", REINSTALL)];

const EVERY_INSTALLER: &str = "  npm install -g @repost/agent-terminal@latest
  pnpm add -g @repost/agent-terminal@latest
  yarn global add @repost/agent-terminal@latest
  bun add -g @repost/agent-terminal@latest
  brew upgrade repost-sh/tap/agent-terminal";

struct Installed {
    installer: &'static str,
    binary: &'static str,
    manager: &'static str,
    answers: &'static [(&'static str, &'static str)],
    calls: &'static str,
}

const INSTALLS: &[Installed] = &[
    Installed {
        installer: "npm",
        binary: NPM_INSTALLED_BINARY,
        manager: "npm",
        answers: NEWER_ON_NPM,
        calls: "npm view @repost/agent-terminal version\nnpm install -g @repost/agent-terminal@latest\n",
    },
    Installed {
        installer: "pnpm",
        binary: "store/v11/links/@repost/agent-terminal-linux-x64/0.0.1/3f9c/node_modules/@repost/agent-terminal-linux-x64/bin/agent-terminal",
        manager: "pnpm",
        answers: NEWER_ON_PNPM,
        calls: "pnpm view @repost/agent-terminal version\npnpm add -g @repost/agent-terminal@latest\n",
    },
    Installed {
        installer: "pnpm",
        binary: "global/5/node_modules/.pnpm/@repost+agent-terminal-linux-x64@0.0.1/node_modules/@repost/agent-terminal-linux-x64/bin/agent-terminal",
        manager: "pnpm",
        answers: NEWER_ON_PNPM,
        calls: "pnpm view @repost/agent-terminal version\npnpm add -g @repost/agent-terminal@latest\n",
    },
    Installed {
        installer: "yarn",
        binary: ".config/yarn/global/node_modules/@repost/agent-terminal-linux-x64/bin/agent-terminal",
        manager: "yarn",
        answers: &[
            ("info @repost/agent-terminal version --silent", "echo 9.9.9"),
            ("global add @repost/agent-terminal@latest", REINSTALL),
        ],
        calls: "yarn info @repost/agent-terminal version --silent\nyarn global add @repost/agent-terminal@latest\n",
    },
    Installed {
        installer: "bun",
        binary: "bun/install/global/node_modules/@repost/agent-terminal-linux-x64/bin/agent-terminal",
        manager: "bun",
        answers: &[
            ("pm view @repost/agent-terminal version", "echo 9.9.9"),
            ("add -g @repost/agent-terminal@latest", REINSTALL),
        ],
        calls: "bun pm view @repost/agent-terminal version\nbun add -g @repost/agent-terminal@latest\n",
    },
    Installed {
        installer: "Homebrew",
        binary: "homebrew/Cellar/agent-terminal/0.0.1/bin/agent-terminal",
        manager: "brew",
        answers: &[
            ("update", "echo 'Already up-to-date.'"),
            (
                "info --json=v2 repost-sh/tap/agent-terminal",
                r#"echo '{"formulae":[{"full_name":"repost-sh/tap/agent-terminal","versions":{"stable":"9.9.9","head":null,"bottle":true}}],"casks":[]}'"#,
            ),
            ("upgrade repost-sh/tap/agent-terminal", REINSTALL),
        ],
        calls: "brew update\nbrew info --json=v2 repost-sh/tap/agent-terminal\nbrew upgrade repost-sh/tap/agent-terminal\n",
    },
];

#[test]
fn upgrade_asks_the_installer_for_the_latest_version_and_reinstalls_with_it() {
    for installed in INSTALLS {
        let install = Install::new(installed.binary);
        install.answering(installed.manager, installed.answers);
        let run = install.upgrade(&["upgrade"]).success();
        assert_eq!(install.calls(), installed.calls);
        assert_eq!(run.stdout(), format!("upgraded agent-terminal {VERSION} to 9.9.9 with {}\n", installed.installer));
        assert!(run.stderr().contains("changed 1 package"), "the installer's output goes to stderr: {}", run.stderr());
        assert!(!install.stub().exists(), "no skill stub was installed, so upgrade writes none");

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
                "stub_refreshed": false,
            })
        );
    }
}

#[test]
fn upgrade_does_not_reinstall_when_the_running_version_is_the_latest() {
    let install = Install::new(NPM_INSTALLED_BINARY);
    install.answering("npm", &[("view @repost/agent-terminal version", &format!("echo {VERSION}"))]);
    put_stub(&install.user, &stub_of_version("0.0.1"));

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
            "stub_refreshed": false,
        })
    );
    assert_eq!(install.calls(), "npm view @repost/agent-terminal version\n".repeat(2));
    assert_eq!(std::fs::read_to_string(install.stub()).unwrap(), stub_of_version("0.0.1"));
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

    let run = install.upgrade(&["--json", "upgrade"]);
    assert_eq!(run.code(), 1);
    assert_eq!(run.json(), serde_json::json!({ "error": { "code": 1, "message": message } }));
    assert_eq!(run.stderr(), npm_said);

    assert_eq!(install.calls(), "npm view @repost/agent-terminal version\n".repeat(2), "nothing is reinstalled");
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

const RELOCATED_BINARY: &str = "global/node_modules/@repost/agent-terminal-linux-x64/bin/agent-terminal";

fn global_listings(install: &Install, npm_lists: &str) {
    install.answering("npm", &[("ls -g --depth=0", &format!("echo '/usr/local/lib'\necho '└── {npm_lists}'"))]);
    install.answering("pnpm", &[("ls -g --depth=0", "echo 'Legend: production dependency, optional only, dev only'")]);
    install.answering(
        "yarn",
        &[
            ("global list", "echo 'info \"@repost/agent-terminal@0.0.1\" has binaries:'\necho '   - agent-terminal'"),
            ("info @repost/agent-terminal version --silent", "echo 9.9.9"),
            ("global add @repost/agent-terminal@latest", REINSTALL),
        ],
    );
    install.answering(
        "bun",
        &[("pm ls -g", "echo '/root/.bun/install/global node_modules (1)'\necho '└── typescript@5.6.3'")],
    );
}

#[test]
fn upgrade_asks_each_installers_global_listing_when_the_path_does_not_tell() {
    let install = Install::new(RELOCATED_BINARY);
    global_listings(&install, "corepack@0.29.4");
    let run = install.upgrade(&["upgrade"]).success();
    assert_eq!(run.stdout(), format!("upgraded agent-terminal {VERSION} to 9.9.9 with yarn\n"));
    assert_eq!(
        install.calls(),
        "npm ls -g --depth=0\npnpm ls -g --depth=0\nyarn global list\nbun pm ls -g\nyarn info @repost/agent-terminal version --silent\nyarn global add @repost/agent-terminal@latest\n"
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
    let refreshed = install.at.run(&["skills", "get", "agent-terminal"]).success().stdout();
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
