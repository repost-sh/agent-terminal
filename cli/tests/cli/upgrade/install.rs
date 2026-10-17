use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::support::{AgentTerminal, Call, Run};

pub(super) struct Install {
    at: AgentTerminal,
    pub(super) binary: PathBuf,
    new_binary: PathBuf,
    pub(super) bin: PathBuf,
    global_bin: PathBuf,
    log: PathBuf,
    pub(super) user: PathBuf,
}

impl Install {
    pub(super) fn new(binary: &str) -> Self {
        let at = AgentTerminal::new();
        let binary = at.home().join(binary);
        std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
        std::fs::hard_link(env!("CARGO_BIN_EXE_agent-terminal"), &binary)
            .expect("link the built binary into the install");
        let bin = at.home().join("fake-bin");
        std::fs::create_dir_all(&bin).unwrap();
        let install = Self {
            new_binary: at.home().join("agent-terminal-9.9.9"),
            global_bin: at.home().join("installer/bin"),
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

    pub(super) fn published_binary(&self, script: &str) {
        executable(&self.new_binary, &format!("#!/bin/sh\n{script}"));
    }

    pub(super) fn package_manager(&self, name: &str, script: &str) {
        executable(&self.bin.join(name), &format!("#!/bin/sh\necho \"{name} $*\" >> \"$CALLS\"\n{script}"));
    }

    pub(super) fn answering(&self, name: &str, answers: &[(&str, &str)]) {
        let arms = answers.iter().fold(String::new(), |mut arms, (args, script)| {
            write!(arms, "'{args}')\n{script}\n;;\n").expect("write a case arm");
            arms
        });
        self.package_manager(name, &format!("case \"$*\" in\n{arms}*)\nexit 64\n;;\nesac\n"));
    }

    pub(super) fn upgrade(&self, args: &[&str]) -> Run {
        let path = format!("{}:/usr/bin:/bin", self.bin.display());
        let env = [
            ("PATH", path.as_str()),
            ("CALLS", &*self.log.to_string_lossy()),
            ("BINARY", &*self.binary.to_string_lossy()),
            ("NEW_BINARY", &*self.new_binary.to_string_lossy()),
            ("GLOBAL_BIN", &*self.global_bin.to_string_lossy()),
            ("HOME", &*self.user.to_string_lossy()),
        ];
        self.at.run_program(&self.binary, &Call { args, stdin: "", env: &env })
    }

    pub(super) fn calls(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
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
