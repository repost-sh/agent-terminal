use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::support::AgentTerminal;

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
    binary: PathBuf,
    bin: PathBuf,
    log: PathBuf,
}

impl Install {
    fn new(at: &AgentTerminal, binary: &str) -> Self {
        let binary = at.home().join(binary);
        std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
        std::fs::copy(env!("CARGO_BIN_EXE_agent-terminal"), &binary).unwrap();
        let bin = at.home().join("fake-bin");
        std::fs::create_dir_all(&bin).unwrap();
        Self { binary, bin, log: at.home().join("package-manager.log") }
    }

    fn package_manager(&self, name: &str, script: &str) {
        let path = self.bin.join(name);
        std::fs::write(&path, format!("#!/bin/sh\necho \"$@\" >> \"$CALLS\"\n{script}")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn upgrade(&self, at: &AgentTerminal, args: &[&str]) -> crate::support::Run {
        let path = format!("{}:/usr/bin:/bin", self.bin.display());
        let env = [
            ("PATH", path.as_str()),
            ("CALLS", &*self.log.to_string_lossy()),
            ("BINARY", &*self.binary.to_string_lossy()),
        ];
        at.run_program(&self.binary, args, "", &env)
    }

    fn calls(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }
}

const NPM_INSTALLED_BINARY: &str =
    "prefix/lib/node_modules/@repost/agent-terminal/node_modules/@repost/agent-terminal-linux-x64/bin/agent-terminal";

const REPLACE_BINARY_WITH_9_9_9: &str = "printf '#!/bin/sh\\necho agent-terminal 9.9.9\\n' > \"$BINARY.new\"\nchmod +x \"$BINARY.new\"\nmv \"$BINARY.new\" \"$BINARY\"\necho 'changed 1 package'\n";

#[test]
fn upgrade_of_an_npm_install_runs_npm_and_reports_the_new_version() {
    let at = AgentTerminal::new();
    let install = Install::new(&at, NPM_INSTALLED_BINARY);
    install.package_manager("npm", REPLACE_BINARY_WITH_9_9_9);
    let run = install.upgrade(&at, &["upgrade"]).success();
    assert_eq!(install.calls(), "install -g @repost/agent-terminal@latest\n");
    assert_eq!(run.stdout(), format!("upgraded agent-terminal {VERSION} to 9.9.9 with npm\n"));
    assert!(run.stderr().contains("changed 1 package"), "npm's own output goes to stderr: {}", run.stderr());
}

#[test]
fn upgrade_of_a_homebrew_install_runs_brew_and_says_when_nothing_changed() {
    let at = AgentTerminal::new();
    let install = Install::new(&at, "homebrew/Cellar/agent-terminal/0.0.1/bin/agent-terminal");
    install.package_manager("brew", "echo 'Warning: agent-terminal already installed'\n");
    let run = install.upgrade(&at, &["upgrade"]).success();
    assert_eq!(install.calls(), "upgrade repost-sh/tap/agent-terminal\n");
    assert_eq!(run.stdout(), format!("agent-terminal {VERSION} is already the latest version\n"));

    let json = install.upgrade(&at, &["--json", "upgrade"]).success().json();
    assert_eq!(json, serde_json::json!({ "installed_with": "Homebrew", "from": VERSION, "to": VERSION }));
}

#[test]
fn upgrade_refuses_a_binary_that_npm_or_homebrew_did_not_install() {
    let at = AgentTerminal::new();
    let run = at.run(&["upgrade"]);
    assert_eq!((run.code(), run.stdout().as_str()), (1, ""));
    let binary = Path::new(env!("CARGO_BIN_EXE_agent-terminal")).canonicalize().unwrap();
    assert_eq!(
        run.stderr(),
        format!(
            "error: agent-terminal at {} was not installed with npm or Homebrew, so upgrade cannot update it; install it with `npm i -g @repost/agent-terminal`, or update it the way you installed it\n",
            binary.display()
        )
    );
}

#[test]
fn upgrade_fails_when_the_package_manager_fails_or_is_missing() {
    let at = AgentTerminal::new();
    let install = Install::new(&at, NPM_INSTALLED_BINARY);

    let run = install.upgrade(&at, &["upgrade"]);
    assert_eq!((run.code(), run.stdout().as_str()), (1, ""));
    assert!(
        run.stderr().starts_with("error: cannot run `npm install -g @repost/agent-terminal@latest`: "),
        "{}",
        run.stderr()
    );

    install.package_manager("npm", "echo 'npm error 404 Not Found' >&2\nexit 1\n");
    let run = install.upgrade(&at, &["upgrade"]);
    assert_eq!((run.code(), run.stdout().as_str()), (1, ""));
    assert_eq!(
        run.stderr(),
        "npm error 404 Not Found\nerror: `npm install -g @repost/agent-terminal@latest` failed (exit status: 1); see its output above\n"
    );
}
