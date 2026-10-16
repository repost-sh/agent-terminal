use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tungstenite::{Message, WebSocket};

const READ_TIMEOUT: Duration = Duration::from_secs(25);

pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) struct AgentTerminal {
    home: tempfile::TempDir,
}

#[derive(Clone, Copy)]
pub(crate) struct Call<'a> {
    pub args: &'a [&'a str],
    pub stdin: &'a str,
    pub env: &'a [(&'a str, &'a str)],
}

pub(crate) struct Shown<'a> {
    pub size: &'a str,
    pub bytes: &'a str,
    pub wait: &'a str,
}

pub(crate) struct Screen {
    dir: tempfile::TempDir,
}

impl Screen {
    pub(crate) fn open(at: &AgentTerminal, shown: &Shown<'_>) -> Self {
        let dir = scratch("screen-");
        let path = dir.path().join("screen");
        std::fs::write(&path, shown.bytes).expect("write the screen");
        let path = path.to_string_lossy().into_owned();
        at.run(&["open", "--size", shown.size, "--wait", shown.wait, "sh", "-c", "cat \"$1\"; read _", "sh", &path])
            .success();
        Self { dir }
    }

    pub(crate) fn file(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
}

impl AgentTerminal {
    pub(crate) fn new() -> Self {
        let home = scratch("at-");
        Self { home }
    }

    pub(crate) fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agent-terminal"));
        command.args(args).env("AGENT_TERMINAL_HOME", self.home.path()).env_remove("AGENT_TERMINAL_SESSION");
        command
    }

    pub(crate) fn run(&self, args: &[&str]) -> Run {
        self.run_with_env(args, &[])
    }

    pub(crate) fn run_with_env(&self, args: &[&str], env: &[(&str, &Path)]) -> Run {
        Run(self.command(args).envs(env.iter().copied()).output().expect("run agent-terminal"))
    }

    pub(crate) fn run_with_session_env(&self, session: &str, args: &[&str]) -> Run {
        Run(self.command(args).env("AGENT_TERMINAL_SESSION", session).output().expect("run agent-terminal"))
    }

    pub(crate) fn run_in(&self, dir: &Path, args: &[&str]) -> Run {
        Run(self.command(args).current_dir(dir).output().expect("run agent-terminal"))
    }

    pub(crate) fn run_with(&self, call: &Call<'_>) -> Run {
        self.run_program(Path::new(env!("CARGO_BIN_EXE_agent-terminal")), call)
    }

    pub(crate) fn run_program(&self, program: &Path, call: &Call<'_>) -> Run {
        let Call { args, stdin, env } = *call;
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

    pub(crate) fn home(&self) -> &Path {
        self.home.path()
    }

    pub(crate) fn frame(&self, session: &str) -> u64 {
        self.run(&["--session", session, "get", "frame"]).success().stdout().trim().parse().expect("a frame number")
    }

    pub(crate) fn work_folder(&self) -> PathBuf {
        let work = self.home.path().join("work");
        std::fs::create_dir_all(&work).expect("create the work folder");
        work.canonicalize().expect("the work folder's canonical path")
    }

    pub(crate) fn activity(&self, session: &str) -> Vec<serde_json::Value> {
        let path = self.home.path().join("sessions").join(session).join("activity.jsonl");
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
            .lines()
            .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}")))
            .collect()
    }

    pub(crate) fn received(&self, session: &str) -> String {
        let snapshot = self.run(&["--session", session, "--json", "snapshot"]).success().json();
        snapshot["lines"][1].as_str().expect("the second row").to_string()
    }

    pub(crate) fn daemon_log(&self) -> String {
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

pub(crate) struct Run(pub Output);

impl Run {
    pub(crate) fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.0.stdout).into_owned()
    }

    pub(crate) fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.0.stderr).into_owned()
    }

    pub(crate) fn code(&self) -> i32 {
        self.0.status.code().unwrap_or(-1)
    }

    #[track_caller]
    pub(crate) fn success(self) -> Self {
        assert_eq!(self.code(), 0, "expected success\nstdout:\n{}\nstderr:\n{}", self.stdout(), self.stderr());
        self
    }

    pub(crate) fn screen(&self) -> String {
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

    pub(crate) fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.0.stdout)
            .unwrap_or_else(|e| panic!("stdout is not JSON ({e}):\n{}", self.stdout()))
    }
}

pub(crate) struct Cue {
    dir: tempfile::TempDir,
    listener: UnixListener,
    fixture: Mutex<Option<UnixStream>>,
}

impl Cue {
    pub(crate) fn new() -> Self {
        let dir = scratch("cue-");
        let listener = UnixListener::bind(dir.path().join("cue")).expect("listen for the fixture's cue connection");
        Self { dir, listener, fixture: Mutex::new(None) }
    }

    pub(crate) fn path(&self) -> String {
        self.dir.path().join("cue").to_string_lossy().into_owned()
    }

    pub(crate) fn print(&self, text: &str) {
        let mut connected = self.fixture.lock().unwrap();
        let fixture =
            connected.get_or_insert_with(|| self.listener.accept().expect("the fixture connects to its cue").0);
        let length = u32::try_from(text.len()).expect("a cue shorter than 4 GiB").to_be_bytes();
        fixture.write_all(&length).and_then(|()| fixture.write_all(text.as_bytes())).expect("send the cue");
        let mut ack = [0u8; 1];
        fixture.read_exact(&mut ack).expect("read the acknowledgement");
        assert_eq!(&ack, b"\n");
    }

    pub(crate) fn open_line_printer(&self, at: &AgentTerminal, options: &[&str]) {
        let printer = fixture("line-printer");
        let path = self.path();
        let mut args = vec!["open"];
        args.extend_from_slice(options);
        args.extend_from_slice(&["--wait", "line 2", &printer, "--lines", "2", "--cue", &path]);
        at.run(&args).success();
        self.print("");
    }
}

pub(crate) struct DashboardSocket {
    socket: WebSocket<TcpStream>,
}

impl DashboardSocket {
    pub(crate) fn open(port: u16, path: &str) -> Self {
        let stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to the dashboard");
        stream.set_read_timeout(Some(READ_TIMEOUT)).expect("bound every read");
        let url = format!("ws://localhost:{port}{path}");
        let (socket, _) = tungstenite::client(url.as_str(), stream).unwrap_or_else(|e| panic!("open {url}: {e}"));
        Self { socket }
    }

    pub(crate) fn next(&mut self) -> serde_json::Value {
        match self.socket.read().expect("a dashboard message") {
            Message::Text(text) => serde_json::from_str(&text).expect("a JSON message"),
            other => panic!("expected a text message, got {other:?}"),
        }
    }

    pub(crate) fn try_next(&mut self, within: Duration) -> Option<serde_json::Value> {
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

    pub(crate) fn closed(&mut self) -> bool {
        match self.socket.read() {
            Ok(Message::Close(_)) | Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::Protocol(_)) => true,
            Ok(Message::Text(_)) => false,
            other => panic!("expected the dashboard to close the socket, got {other:?}"),
        }
    }

    pub(crate) fn send(&mut self, message: &serde_json::Value) {
        self.socket.send(Message::text(message.to_string())).expect("send a dashboard message");
    }

    pub(crate) fn acknowledge(&mut self, message: &serde_json::Value) -> u64 {
        let frame = frame_number(message);
        self.send(&serde_json::json!({ "type": "shown" }));
        frame
    }

    pub(crate) fn acknowledge_next(&mut self) -> u64 {
        let message = self.next();
        self.acknowledge(&message)
    }
}

pub(crate) fn frame_number(message: &serde_json::Value) -> u64 {
    assert_eq!(message["type"], "frame", "{message}");
    message["frame"].as_u64().expect("a frame number")
}

pub(crate) struct Redraw<'a> {
    pub session: &'a str,
    pub hz: &'a str,
    pub repaint: &'a str,
}

pub(crate) fn open_redraw(at: &AgentTerminal, redraw: &Redraw<'_>) {
    let Redraw { session, hz, repaint } = *redraw;
    at.run(&[
        "--session",
        session,
        "open",
        "--size",
        "120x40",
        "--wait",
        "forward",
        &fixture("redraw"),
        "--hz",
        hz,
        repaint,
    ])
    .success();
}

pub(crate) fn dashboard(at: &AgentTerminal) -> u16 {
    let started = at.run(&["--json", "dashboard", "start", "--port", "0"]).success().json();
    u16::try_from(started["port"].as_u64().expect("the dashboard's port")).expect("a port number")
}

pub(crate) fn fixture(name: &str) -> String {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    let dir = BUILT.get_or_init(|| {
        let bins = PathBuf::from(env!("CARGO_BIN_EXE_agent-terminal")).parent().unwrap().to_path_buf();
        let status = Command::new(env!("CARGO"))
            .args(["build", "--quiet", "-p", "at-fixtures", "--bins", "--target-dir"])
            .arg(bins.parent().unwrap())
            .status()
            .expect("build fixtures");
        assert!(status.success(), "building fixtures failed");
        bins
    });
    dir.join(name).to_string_lossy().into_owned()
}

pub(crate) fn scratch(prefix: &str) -> tempfile::TempDir {
    tempfile::Builder::new().prefix(prefix).tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("scratch folder")
}
