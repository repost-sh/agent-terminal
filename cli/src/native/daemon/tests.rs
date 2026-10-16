use std::fs::File;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak, mpsc};
use std::time::Duration;

use serde_json::{Value, json};

use super::Daemon;
use super::refusals::taken;
use crate::home::DAEMON_LOCK;
use crate::protocol;
use crate::protocol::SessionInfo;

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
        let claim = File::create(home.path().join(DAEMON_LOCK)).unwrap();
        claim.lock().unwrap();
        let daemon = Arc::new_cyclic(|me| Daemon::new(Weak::clone(me), ended, at, claim));
        let running = Self { daemon, home, ended: end };
        running.ok(&json!({ "command": "new-session", "session": "s" }));
        running
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
        (refusal["code"].as_i64().unwrap(), shown.replace(crate::version::BUILD, "<build>"))
    }

    pub(super) fn open(&self, tab: &str, script: &str) -> Value {
        self.ok(&open(tab, script))
    }

    pub(super) fn user(&self) -> PathBuf {
        let user = self.home.path().join("user");
        std::fs::create_dir_all(&user).unwrap();
        user
    }

    pub(super) fn shell(&self, tab: &str, timeout_ms: u64) -> Value {
        let user = self.user();
        json!({
            "command": "shell",
            "session": "s",
            "tab": tab,
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
        self.daemon.stop_dashboard();
        self.daemon.close_all();
    }
}

pub(super) fn open(tab: &str, script: &str) -> Value {
    json!({
        "command": "open",
        "session": "s",
        "tab": tab,
        "program": "/bin/sh",
        "args": ["-c", script],
        "size": { "cols": 30, "rows": 4 },
        "cwd": Path::new("/"),
        "env": [["PATH", "/usr/bin:/bin"]],
        "wait": text("ready"),
        "timeout_ms": 10_000
    })
}

pub(super) fn waiting(mut request: Value, wait: Value) -> Value {
    request["wait"] = wait;
    request
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
    assert_eq!(
        running.ok(&json!({ "command": "sessions" })),
        json!({ "sessions": [{ "session": "s", "current": null, "tabs": [] }] })
    );
    running.open("echo", ECHO);
    let listed = running.ok(&json!({ "command": "sessions" }));
    insta::assert_snapshot!(listed, @r#"{"sessions":[{"current":"echo","session":"s","tabs":[{"cols":30,"command":"/bin/sh -c \"printf 'ready\\\\n'; while read line; do printf 'got %s\\\\n' \\\"\"'$line\"; done'","control":"agent","cwd":"/","exit_code":null,"exited":false,"program":"sh","rows":4,"tab":"echo"}]}]}"#);
}

#[test]
fn a_session_name_is_taken_until_close_all_ends_its_session() {
    let running = Running::new();
    let new = json!({ "command": "new-session", "session": "s" });
    let mut held = vec![running.refused(&new).1];
    running.open("app", ECHO);
    held.push(running.refused(&new).1);
    running.open("api", ECHO);
    running.ok(&on("close", &json!({ "scope": "tab", "tab": "app" })));
    held.push(running.refused(&new).1);
    let held: Vec<&str> = held.iter().map(|message| message.split_once(", ").map_or("", |(_, tabs)| tabs)).collect();
    assert_eq!(
        held,
        [
            "no tabs; pick another name for this conversation",
            "1 tab: app; pick another name for this conversation",
            "1 tab: api; pick another name for this conversation"
        ]
    );
    running.ok(&json!({ "command": "close", "scope": "session", "session": "s" }));
    assert_eq!(running.ok(&new), json!({ "session": "s" }));
}

#[test]
fn a_taken_name_says_how_long_its_session_has_been_open_and_which_tabs_it_has() {
    let held = |tabs: &[&str]| -> SessionInfo {
        let tabs: Vec<Value> = tabs
            .iter()
            .map(|tab| json!({ "tab": tab, "program": "sh", "command": "sh", "cwd": "/", "cols": 2, "rows": 2, "exited": false, "exit_code": null, "control": "agent" }))
            .collect();
        serde_json::from_value(json!({ "session": "api", "current": null, "tabs": tabs })).unwrap()
    };
    let ages = [0, 59, 60, 3599, 3600, 7322].map(|seconds| taken(&held(&[]), Duration::from_secs(seconds)).message);
    assert_eq!(
        ages.map(|message| message.trim_end_matches(", no tabs; pick another name for this conversation").to_string()),
        [
            "session api is taken: open for 0s",
            "session api is taken: open for 59s",
            "session api is taken: open for 1m",
            "session api is taken: open for 59m",
            "session api is taken: open for 1h 0m",
            "session api is taken: open for 2h 2m"
        ]
    );
    assert_eq!(
        taken(&held(&["app", "web", "db"]), Duration::from_secs(5)).message,
        "session api is taken: open for 5s, 3 tabs: app, web, db; pick another name for this conversation"
    );
}

#[test]
fn a_request_the_daemon_cannot_read_is_answered_with_its_build() {
    let running = Running::new();
    let (code, message) = running.refused(&json!({ "command": "teleport" }));
    assert_eq!(code, 6);
    insta::assert_snapshot!(message, @"the daemon is agent-terminal <build> and does not understand this request, so the command is another build; it has 0 open terminals: a human restarts the daemon when it suits them, because it holds other agents' terminals");
}

#[test]
fn a_tab_or_session_that_is_not_there_is_named_in_the_refusal() {
    let running = Running::new();
    let snapshot = |tab: Option<&str>| on("snapshot", &json!({ "tab": tab }));
    let mut elsewhere = snapshot(None);
    elsewhere["session"] = json!("nowhere");
    assert_eq!(
        [running.refused(&snapshot(None)), running.refused(&snapshot(Some("web"))), running.refused(&elsewhere)],
        [
            (
                1,
                "no terminal is open in session s; start one with `agent-terminal open --name <name> <program>`"
                    .to_string()
            ),
            (1, "no tab web in session s; list tabs with `agent-terminal tab`".to_string()),
            (1, "no session nowhere; create it with `agent-terminal session new nowhere`".to_string())
        ]
    );
}

#[test]
fn attach_is_refused_anywhere_but_its_own_connection() {
    let running = Running::new();
    let attach = json!({ "command": "batch", "bail": false, "steps": [{ "args": ["attach"], "request": { "command": "attach", "session": "s", "tab": null, "read_only": false } }] });
    insta::assert_snapshot!(running.ok(&attach), @r#"[{"args":["attach"],"response":{"Err":{"code":1,"message":"attach shows the tab in your terminal, so it runs on its own, not in batch or from the dashboard"}}}]"#);
}
