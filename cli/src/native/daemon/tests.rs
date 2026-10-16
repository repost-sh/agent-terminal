use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak, mpsc};

use serde_json::{Value, json};

use super::Daemon;
use crate::protocol;

pub(super) struct Running {
    pub daemon: Arc<Daemon>,
    pub home: tempfile::TempDir,
    pub ended: mpsc::Receiver<()>,
}

impl Running {
    pub(super) fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let (ended, end) = mpsc::channel();
        let at = home.path().to_path_buf();
        let daemon = Arc::new_cyclic(|me| Daemon::new(Weak::clone(me), ended, at));
        Self { daemon, home, ended: end }
    }

    pub(super) fn respond(&self, typed: Option<&[&str]>, request: &Value) -> Value {
        let (mut client, server) = UnixStream::pair().unwrap();
        let daemon = Arc::clone(&self.daemon);
        let serving = std::thread::spawn(move || daemon.serve(server));
        protocol::write(&mut client, &json!({ "typed": typed, "request": request })).unwrap();
        let response = protocol::read::<Value>(&mut client).unwrap().unwrap();
        serving.join().unwrap();
        response
    }

    pub(super) fn ok(&self, request: &Value) -> Value {
        let response = self.respond(None, request);
        assert!(response.get("Ok").is_some(), "{response}");
        response["Ok"]["data"].clone()
    }

    pub(super) fn refused(&self, request: &Value) -> (i64, String) {
        let response = self.respond(None, request);
        let refusal = &response["Err"];
        let shown = refusal["message"].as_str().unwrap_or_else(|| panic!("{response}"));
        let shown = shown.replace(&*self.home.path().to_string_lossy(), "<home>");
        (refusal["code"].as_i64().unwrap(), shown.replace(crate::version::VERSION, "<version>"))
    }

    pub(super) fn open(&self, script: &str) -> Value {
        self.ok(&open(script, &text("ready")))
    }

    pub(super) fn user(&self) -> PathBuf {
        let user = self.home.path().join("user");
        std::fs::create_dir_all(&user).unwrap();
        user
    }

    pub(super) fn shell(&self, timeout_ms: u64) -> Value {
        let user = self.user();
        json!({
            "command": "shell",
            "session": "s",
            "shell": "bash",
            "program": "bash",
            "size": { "cols": 60, "rows": 8 },
            "cwd": user,
            "env": [["PATH", "/usr/bin:/bin"], ["HOME", user]],
            "wait": { "until": "command" },
            "timeout_ms": timeout_ms
        })
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.daemon.close_all();
    }
}

pub(super) fn open(script: &str, wait: &Value) -> Value {
    json!({
        "command": "open",
        "session": "s",
        "program": "/bin/sh",
        "args": ["-c", script],
        "size": { "cols": 30, "rows": 4 },
        "cwd": Path::new("/"),
        "env": [["PATH", "/usr/bin:/bin"]],
        "wait": wait,
        "timeout_ms": 10_000
    })
}

pub(super) fn text(text: &str) -> Value {
    json!({ "until": "text", "text": text, "regex": false, "rows": null })
}

pub(super) fn on(command: &str, rest: &Value) -> Value {
    let mut request = json!({ "command": command, "session": "s", "tab": null });
    if let (Some(fields), Some(more)) = (request.as_object_mut(), rest.as_object()) {
        fields.extend(more.clone());
    }
    request
}

pub(super) const ECHO: &str = "printf 'ready\\n'; while read line; do printf 'got %s\\n' \"$line\"; done";

#[test]
fn sessions_list_each_tab_with_its_program_size_and_owner() {
    let running = Running::new();
    assert_eq!(running.ok(&json!({ "command": "sessions" })), json!({ "sessions": [] }));
    running.open(ECHO);
    let listed = running.ok(&json!({ "command": "sessions" }));
    insta::assert_snapshot!(listed, @r#"{"sessions":[{"current":"t1","session":"s","tabs":[{"cols":30,"command":"/bin/sh -c \"printf 'ready\\\\n'; while read line; do printf 'got %s\\\\n' \\\"\"'$line\"; done'","control":"agent","cwd":"/","exit_code":null,"exited":false,"program":"sh","rows":4,"tab":"t1"}]}]}"#);
}

#[test]
fn a_request_the_daemon_cannot_read_is_answered_with_its_version() {
    let running = Running::new();
    let (code, message) = running.refused(&json!({ "command": "teleport" }));
    assert_eq!(code, 6);
    insta::assert_snapshot!(message, @"the daemon is agent-terminal <version> and does not understand this request, so the command is another version; it has 0 open terminals: `agent-terminal daemon stop` ends it and its terminals, and the next command starts a daemon of the command's version");
}

#[test]
fn a_tab_or_session_that_is_not_there_is_named_in_the_refusal() {
    let running = Running::new();
    let snapshot = |tab: Option<&str>| on("snapshot", &json!({ "tab": tab }));
    assert_eq!(
        [running.refused(&snapshot(None)), running.refused(&snapshot(Some("t3")))],
        [
            (1, "no terminal is open in session s; start one with `agent-terminal open <program>`".to_string()),
            (1, "no tab t3 in session s; list tabs with `agent-terminal tab`".to_string())
        ]
    );
}

#[test]
fn attach_is_refused_anywhere_but_its_own_connection() {
    let running = Running::new();
    let attach = json!({ "command": "batch", "bail": false, "steps": [{ "args": ["attach"], "request": { "command": "attach", "session": "s", "tab": null, "read_only": false } }] });
    insta::assert_snapshot!(running.ok(&attach), @r#"[{"args":["attach"],"response":{"Err":{"code":1,"message":"attach shows the tab in your terminal, so it runs on its own, not in batch or from the dashboard"}}}]"#);
}
