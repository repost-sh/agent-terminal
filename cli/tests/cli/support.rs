use std::path::{Path, PathBuf};
use std::process::{Command, Output};
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
        self.run_with_env(args, &[])
    }

    pub fn run_with_env(&self, args: &[&str], env: &[(&str, &Path)]) -> Run {
        let output = Command::new(env!("CARGO_BIN_EXE_agent-terminal"))
            .args(args)
            .env("AGENT_TERMINAL_HOME", self.home.path())
            .env_remove("AGENT_TERMINAL_SESSION")
            .envs(env.iter().copied())
            .output()
            .expect("run agent-terminal");
        Run(output)
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
        let out = self.stdout();
        match out.find("frame ") {
            Some(i) => {
                let end = out[i + 6..].find(|c: char| !c.is_ascii_digit()).map_or(out.len(), |e| i + 6 + e);
                format!("{}frame N{}", &out[..i], &out[end..])
            }
            None => out,
        }
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.0.stdout)
            .unwrap_or_else(|e| panic!("stdout is not JSON ({e}):\n{}", self.stdout()))
    }
}

pub struct Cue {
    dir: tempfile::TempDir,
}

impl Cue {
    pub fn new() -> Self {
        let dir = tempfile::Builder::new().prefix("cue-").tempdir_in("/tmp").expect("temp cue folder");
        for fifo in ["cue", "ack"] {
            let status = Command::new("mkfifo").arg(dir.path().join(fifo)).status().expect("run mkfifo");
            assert!(status.success(), "mkfifo {fifo} failed");
        }
        Self { dir }
    }

    pub fn dir(&self) -> String {
        self.dir.path().to_string_lossy().into_owned()
    }

    pub fn print(&self, text: &str) {
        std::fs::write(self.dir.path().join("cue"), text).expect("send the cue");
        let ack = std::fs::read_to_string(self.dir.path().join("ack")).expect("read the acknowledgement");
        assert_eq!(ack, "printed\n");
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
