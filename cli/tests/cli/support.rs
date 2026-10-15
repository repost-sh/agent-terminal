use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tungstenite::{Message, WebSocket};

const READ_TIMEOUT: Duration = Duration::from_secs(25);

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct AgentTerminal {
    home: tempfile::TempDir,
}

impl AgentTerminal {
    pub fn new() -> Self {
        let home = scratch("at-");
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

    pub fn frame(&self, session: &str) -> u64 {
        self.run(&["--session", session, "get", "frame"]).success().stdout().trim().parse().expect("a frame number")
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
        let dir = scratch("cue-");
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

pub struct DashboardSocket {
    socket: WebSocket<TcpStream>,
}

impl DashboardSocket {
    pub fn open(port: u16, path: &str) -> Self {
        let stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to the dashboard");
        stream.set_read_timeout(Some(READ_TIMEOUT)).expect("bound every read");
        let url = format!("ws://localhost:{port}{path}");
        let (socket, _) = tungstenite::client(url.as_str(), stream).unwrap_or_else(|e| panic!("open {url}: {e}"));
        Self { socket }
    }

    pub fn next(&mut self) -> serde_json::Value {
        match self.socket.read().expect("a dashboard message") {
            Message::Text(text) => serde_json::from_str(&text).expect("a JSON message"),
            other => panic!("expected a text message, got {other:?}"),
        }
    }

    pub fn try_next(&mut self, within: Duration) -> Option<serde_json::Value> {
        self.socket.get_ref().set_read_timeout(Some(within)).expect("bound the read");
        let read = self.socket.read();
        self.socket.get_ref().set_read_timeout(Some(READ_TIMEOUT)).expect("bound every read");
        match read {
            Ok(Message::Text(text)) => Some(serde_json::from_str(&text).expect("a JSON message")),
            Ok(other) => panic!("expected a text message, got {other:?}"),
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => None,
            Err(e) => panic!("reading a dashboard message failed: {e}"),
        }
    }

    pub fn send(&mut self, message: &serde_json::Value) {
        self.socket.send(Message::text(message.to_string())).expect("send a dashboard message");
    }

    pub fn acknowledge(&mut self, message: &serde_json::Value) -> u64 {
        let frame = frame_number(message);
        self.send(&serde_json::json!({ "type": "shown" }));
        frame
    }

    pub fn acknowledge_next(&mut self) -> u64 {
        let message = self.next();
        self.acknowledge(&message)
    }
}

pub fn frame_number(message: &serde_json::Value) -> u64 {
    assert_eq!(message["type"], "frame", "{message}");
    message["frame"].as_u64().expect("a frame number")
}

pub fn open_redraw(at: &AgentTerminal, session: &str, hz: &str, repaint: &str) {
    at.run(&[
        "--session",
        session,
        "open",
        "--size",
        "120x40",
        "--wait",
        "ready",
        &fixture("redraw"),
        "--hz",
        hz,
        repaint,
    ])
    .success();
}

pub fn dashboard(at: &AgentTerminal) -> u16 {
    let started = at.run(&["--json", "dashboard", "start", "--port", "0"]).success().json();
    started["port"].as_u64().expect("the dashboard's port") as u16
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

pub fn scratch(prefix: &str) -> tempfile::TempDir {
    tempfile::Builder::new().prefix(prefix).tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("scratch folder")
}
