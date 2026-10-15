use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Mutex, OnceLock};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct AgentTerminal {
    home: tempfile::TempDir,
}

impl AgentTerminal {
    pub fn new() -> Self {
        let home = tempfile::Builder::new().prefix("at-").tempdir_in("/tmp").expect("temp home");
        Self { home }
    }

    pub fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agent-terminal"));
        command.args(args).env("AGENT_TERMINAL_HOME", self.home.path()).env_remove("AGENT_TERMINAL_SESSION");
        command
    }

    pub fn run(&self, args: &[&str]) -> Run {
        self.run_with_env(args, &[])
    }

    pub fn run_with_env(&self, args: &[&str], env: &[(&str, &Path)]) -> Run {
        Run(self.command(args).envs(env.iter().copied()).output().expect("run agent-terminal"))
    }

    pub fn run_with_session_env(&self, session: &str, args: &[&str]) -> Run {
        Run(self.command(args).env("AGENT_TERMINAL_SESSION", session).output().expect("run agent-terminal"))
    }

    pub fn run_in(&self, dir: &Path, args: &[&str]) -> Run {
        Run(self.command(args).current_dir(dir).output().expect("run agent-terminal"))
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

    pub fn work_folder(&self) -> PathBuf {
        let work = self.home.path().join("work");
        std::fs::create_dir_all(&work).expect("create the work folder");
        work.canonicalize().expect("the work folder's canonical path")
    }

    pub fn activity(&self, session: &str) -> Vec<serde_json::Value> {
        let path = self.home.path().join("sessions").join(session).join("activity.jsonl");
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
            .lines()
            .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}")))
            .collect()
    }

    pub fn received(&self, session: &str) -> String {
        let snapshot = self.run(&["--session", session, "--json", "snapshot"]).success().json();
        snapshot["lines"][1].as_str().expect("the second row").to_string()
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

pub struct Cue {
    dir: tempfile::TempDir,
    listener: UnixListener,
    fixture: Mutex<Option<UnixStream>>,
}

impl Cue {
    pub fn new() -> Self {
        let dir = tempfile::Builder::new().prefix("cue-").tempdir_in("/tmp").expect("temp cue folder");
        let listener = UnixListener::bind(dir.path().join("cue")).expect("listen for the fixture's cue connection");
        Self { dir, listener, fixture: Mutex::new(None) }
    }

    pub fn path(&self) -> String {
        self.dir.path().join("cue").to_string_lossy().into_owned()
    }

    pub fn print(&self, text: &str) {
        let mut connected = self.fixture.lock().unwrap();
        let fixture =
            connected.get_or_insert_with(|| self.listener.accept().expect("the fixture connects to its cue").0);
        let length = u32::try_from(text.len()).expect("a cue shorter than 4 GiB").to_be_bytes();
        fixture.write_all(&length).and_then(|()| fixture.write_all(text.as_bytes())).expect("send the cue");
        let mut ack = [0u8; 1];
        fixture.read_exact(&mut ack).expect("read the acknowledgement");
        assert_eq!(&ack, b"\n");
    }

    pub fn restarted(&self) {
        self.fixture.lock().unwrap().take();
    }

    pub fn open_line_printer(&self, at: &AgentTerminal, options: &[&str]) {
        let printer = fixture("line-printer");
        let path = self.path();
        let mut args = vec!["open"];
        args.extend_from_slice(options);
        args.extend_from_slice(&["--wait", "line 2", &printer, "--lines", "2", "--cue", &path]);
        at.run(&args).success();
        self.print("");
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

pub fn snapshot_rows(at: &AgentTerminal, args: &[&str]) -> (Vec<String>, u64) {
    let mut call = vec!["--json"];
    call.extend_from_slice(args);
    call.push("snapshot");
    let snapshot = at.run(&call).success().json();
    let mut rows: Vec<String> = snapshot["lines"]
        .as_array()
        .expect("snapshot lines")
        .iter()
        .map(|row| row.as_str().unwrap().to_string())
        .collect();
    let shown = rows.iter().rposition(|row| !row.is_empty()).map_or(0, |last| last + 1);
    rows.truncate(shown);
    (rows, snapshot["frame"].as_u64().expect("the frame number"))
}
