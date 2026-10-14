use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;

pub struct AgentTerminal {
    home: tempfile::TempDir,
}

impl AgentTerminal {
    pub fn new() -> Self {
        let home = tempfile::Builder::new().prefix("at-").tempdir_in("/tmp").expect("temp home");
        Self { home }
    }

    pub fn run(&self, args: &[&str]) -> Run {
        let output = Command::new(env!("CARGO_BIN_EXE_agent-terminal"))
            .args(args)
            .env("AGENT_TERMINAL_HOME", self.home.path())
            .env_remove("AGENT_TERMINAL_SESSION")
            .output()
            .expect("run agent-terminal");
        Run(output)
    }

    pub fn run_with(&self, args: &[&str], stdin: &str, env: &[(&str, &str)]) -> Run {
        self.run_program(Path::new(env!("CARGO_BIN_EXE_agent-terminal")), args, stdin, env)
    }

    pub fn run_program(&self, program: &Path, args: &[&str], stdin: &str, env: &[(&str, &str)]) -> Run {
        let mut child = Command::new(program)
            .args(args)
            .env("AGENT_TERMINAL_HOME", self.home.path())
            .env_remove("AGENT_TERMINAL_SESSION")
            .envs(env.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("run agent-terminal");
        child.stdin.take().expect("stdin").write_all(stdin.as_bytes()).expect("write stdin");
        Run(child.wait_with_output().expect("wait for agent-terminal"))
    }

    pub fn home(&self) -> &std::path::Path {
        self.home.path()
    }

    pub fn daemon_log(&self) -> String {
        let path = self.home.path().join("daemon.log");
        std::fs::read_to_string(&path).unwrap_or_else(|e| format!("(could not read {}: {e})", path.display()))
    }
}

impl Drop for AgentTerminal {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("--- daemon.log ---\n{}", self.daemon_log());
        }
        let _ = Command::new(env!("CARGO_BIN_EXE_agent-terminal"))
            .args(["daemon", "stop"])
            .env("AGENT_TERMINAL_HOME", self.home.path())
            .output();
    }
}

pub struct Run(pub Output);

impl Run {
    pub fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.0.stdout).into_owned()
    }

    pub fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.0.stderr).into_owned()
    }

    pub fn code(&self) -> i32 {
        self.0.status.code().unwrap_or(-1)
    }

    #[track_caller]
    pub fn success(self) -> Self {
        assert_eq!(self.code(), 0, "expected success\nstdout:\n{}\nstderr:\n{}", self.stdout(), self.stderr());
        self
    }

    pub fn screen(&self) -> String {
        let mut out = self.stdout();
        let mut from = 0;
        while let Some(found) = out[from..].find("frame ") {
            let digits = from + found + 6;
            let end = out[digits..].find(|c: char| !c.is_ascii_digit()).map_or(out.len(), |e| digits + e);
            out.replace_range(digits..end, "N");
            from = digits + 1;
        }
        out
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.0.stdout)
            .unwrap_or_else(|e| panic!("stdout is not JSON ({e}):\n{}", self.stdout()))
    }
}

pub fn fixture(name: &str) -> String {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    let dir = BUILT.get_or_init(|| {
        let status = Command::new(env!("CARGO"))
            .args(["build", "--quiet", "-p", "at-fixtures", "--bins"])
            .status()
            .expect("build fixtures");
        assert!(status.success(), "building fixtures failed");
        PathBuf::from(env!("CARGO_BIN_EXE_agent-terminal")).parent().unwrap().to_path_buf()
    });
    dir.join(name).to_string_lossy().into_owned()
}
