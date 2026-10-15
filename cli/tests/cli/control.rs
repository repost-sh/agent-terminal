use crate::support::{AgentTerminal, Run, fixture};

const BINARY: &str = env!("CARGO_BIN_EXE_agent-terminal");

fn open_app(at: &AgentTerminal) {
    at.run(&["--session", "app", "open", "--size", "40x6", "--wait", "ready", &fixture("input-echo")]).success();
}

fn attach(at: &AgentTerminal, from: &str, extra: &[&str], ready: &str) -> Run {
    let mut args = vec!["--session", from, "open", "--size", "70x10", "--wait", ready, BINARY, "--session", "app"];
    args.push("attach");
    args.extend_from_slice(extra);
    at.run(&args)
}

#[test]
fn while_a_human_is_attached_input_exits_4_and_wait_control_resumes_once_they_detach() {
    let at = AgentTerminal::new();
    open_app(&at);
    attach(&at, "human", &[], "ctrl-p ctrl-q detaches").success();

    let refused = at.run(&["--session", "app", "press", "j"]);
    assert_eq!(refused.code(), 4, "{}", refused.stderr());
    assert_eq!(refused.stderr(), "error: A human has control of app t1. Run `agent-terminal wait --control`.\n");
    let typed = at.run(&["--session", "app", "--json", "type", "x"]);
    assert_eq!(typed.code(), 4);
    assert_eq!(
        typed.json(),
        serde_json::json!({ "error": {
            "code": 4,
            "message": "A human has control of app t1. Run `agent-terminal wait --control`."
        } })
    );
    assert_eq!(at.run(&["--session", "app", "signal", "INT"]).code(), 4);

    let held = at.run(&["--session", "app", "--timeout", "300", "wait", "--control"]);
    assert_eq!(held.code(), 3);
    assert_eq!(
        held.stderr(),
        "error: timed out after 300 ms waiting for a human to hand back control of app t1; wait longer with --timeout <ms>\n"
    );

    std::thread::scope(|scope| {
        let waiting = scope.spawn(|| at.run(&["--session", "app", "--json", "wait", "--control"]));
        at.run(&["--session", "human", "press", "Ctrl+p", "Ctrl+q"]).success();
        let waited = waiting.join().expect("the waiting call").success().json();
        assert_eq!(waited["until"], "a human to hand back control");
        assert_eq!((&waited["session"], &waited["tab"]), (&serde_json::json!("app"), &serde_json::json!("t1")));
    });

    at.run(&["--session", "human", "wait", "--exit"]).success();
    let detached = at.run(&["--session", "human", "get", "text"]).success().stdout();
    assert!(detached.contains("detached from app t1"), "{detached}");
    at.run(&["--session", "app", "press", "j", "--wait", "j"]).success();
    assert_eq!(at.received("app"), "j");

    let refusals: Vec<serde_json::Value> = at.activity("app").into_iter().filter(|entry| entry["exit"] == 4).collect();
    let commands: Vec<&serde_json::Value> = refusals.iter().map(|entry| &entry["command"]).collect();
    assert_eq!(commands, ["--session app press j", "--session app --json type x", "--session app signal INT"]);
    for refusal in &refusals {
        assert_eq!(refusal["tab"], "t1");
        assert_eq!(refusal["error"], "A human has control of app t1. Run `agent-terminal wait --control`.");
    }
}

#[test]
fn wait_control_returns_at_once_while_the_agent_has_control() {
    let at = AgentTerminal::new();
    open_app(&at);
    let waited = at.run(&["--session", "app", "wait", "--control"]).success();
    let line = waited.stdout();
    assert!(line.starts_with("waited "), "{line}");
    assert!(line.ends_with(" ms for a human to hand back control in app t1\n"), "{line}");
}

#[test]
fn the_tab_list_says_when_a_human_has_control() {
    let at = AgentTerminal::new();
    open_app(&at);
    let tabs = at.run(&["--session", "app", "--json", "tab"]).success().json();
    assert_eq!(tabs["tabs"][0]["control"], "agent");
    attach(&at, "human", &[], "ctrl-p ctrl-q detaches").success();

    let tabs = at.run(&["--session", "app", "--json", "tab"]).success().json();
    assert_eq!(tabs["tabs"][0]["control"], "human");
    let listed = at.run(&["--session", "app", "tab"]).success().stdout();
    assert_eq!(listed, "app t1 · input-echo · 40x6 · running · human control · current\n");
}

#[test]
fn closing_the_tab_ends_the_attach() {
    let at = AgentTerminal::new();
    open_app(&at);
    attach(&at, "human", &[], "ctrl-p ctrl-q detaches").success();
    at.run(&["--session", "app", "close"]).success();
    at.run(&["--session", "human", "wait", "--exit"]).success();
    let ended = at.run(&["--session", "human", "get", "text"]).success().stdout();
    assert!(ended.contains("app t1 was closed; detached"), "{ended}");
}

fn human_entries(at: &AgentTerminal) -> Vec<serde_json::Value> {
    at.activity("app").into_iter().filter(|entry| entry.get("human").is_some()).collect()
}

#[test]
fn an_attached_human_types_with_keys_encoded_for_the_program_and_each_key_is_logged_as_human() {
    let at = AgentTerminal::new();
    at.run(&["--session", "app", "open", "--size", "40x6", "--wait", "ready", &fixture("input-echo"), "--app-cursor"])
        .success();
    attach(&at, "human", &[], "ctrl-p ctrl-q detaches").success();

    at.run(&["--session", "human", "type", "hé"]).success();
    let keys = ["Up", "Enter", "Ctrl+c", "Shift+Tab", "Alt+x", "F5", "Ctrl+p", "Down"];
    let mut press = vec!["--session", "human", "press"];
    press.extend(keys);
    at.run(&press).success();
    at.run(&["--session", "app", "wait", r"h\xc3\xa9\eOA\r\x03\e[Z\ex\e[15~\x10\eOB"]).success();
    assert_eq!(at.received("app"), r"h\xc3\xa9\eOA\r\x03\e[Z\ex\e[15~\x10\eOB");

    let entries = human_entries(&at);
    let mut typed = String::new();
    let mut pressed = Vec::new();
    for entry in &entries {
        assert_eq!((&entry["tab"], &entry["exit"]), (&serde_json::json!("t1"), &serde_json::json!(0)), "{entry}");
        match (&entry["human"]["typed"], &entry["human"]["pressed"]) {
            (serde_json::Value::String(text), _) => {
                assert!(entry["command"].as_str().unwrap().starts_with("type "), "{entry}");
                assert!(pressed.is_empty(), "typing came after a key: {entries:?}");
                typed.push_str(text);
            }
            (_, serde_json::Value::String(key)) => {
                assert_eq!(entry["command"], format!("press {key}"), "{entry}");
                pressed.push(key.clone());
            }
            _ => panic!("a human entry types or presses: {entry}"),
        }
    }
    assert_eq!(typed, "hé");
    assert_eq!(pressed, keys);
}

#[test]
fn attach_mirrors_the_tab_with_a_line_saying_how_to_detach() {
    let at = AgentTerminal::new();
    at.run(&["--session", "app", "open", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui")]).success();
    attach(&at, "watcher", &["--read-only"], "read-only").success();

    let inner = at.run(&["--session", "app", "--json", "snapshot"]).success().json();
    let outer = at.run(&["--session", "watcher", "--json", "snapshot"]).success().json();
    let mirrored: Vec<&str> = outer["lines"].as_array().unwrap().iter().map(|line| line.as_str().unwrap()).collect();
    let shown: Vec<&str> = inner["lines"].as_array().unwrap().iter().map(|line| line.as_str().unwrap()).collect();
    assert_eq!(mirrored[..8].iter().map(|line| line.trim_end()).collect::<Vec<_>>(), shown);
    assert_eq!(mirrored[8], "app t1 · read-only · ctrl-p ctrl-q detaches");

    at.run(&["--session", "app", "press", "j", "--wait", "Selected: beta.txt"]).success();
    at.run(&["--session", "watcher", "wait", "Selected: beta.txt"]).success();
}

#[test]
fn a_read_only_attach_leaves_control_with_the_agent_and_sends_no_keys() {
    let at = AgentTerminal::new();
    open_app(&at);
    attach(&at, "watcher", &["--read-only"], "read-only").success();

    let tabs = at.run(&["--session", "app", "--json", "tab"]).success().json();
    assert_eq!(tabs["tabs"][0]["control"], "agent");
    at.run(&["--session", "app", "type", "x", "--wait", "x"]).success();
    at.run(&["--session", "watcher", "wait", "x"]).success();

    at.run(&["--session", "watcher", "type", "y"]).success();
    at.run(&["--session", "watcher", "press", "Ctrl+p", "Ctrl+q"]).success();
    at.run(&["--session", "watcher", "wait", "--exit"]).success();
    assert_eq!(at.received("app"), "x");
    assert!(human_entries(&at).is_empty());
}

#[test]
fn exec_is_refused_while_a_human_has_control_of_the_shell() {
    let at = AgentTerminal::new();
    let home = tempfile::Builder::new().prefix("home-").tempdir_in("/tmp").expect("temp home");
    let shell = at
        .command(&["--session", "app", "shell", "--shell", "bash", "--size", "40x6"])
        .env("HOME", home.path())
        .output()
        .expect("run agent-terminal");
    assert!(shell.status.success(), "{}", String::from_utf8_lossy(&shell.stderr));
    attach(&at, "human", &[], "ctrl-p ctrl-q detaches").success();

    let refused = at.run(&["--session", "app", "exec", "echo hi"]);
    assert_eq!(refused.code(), 4);
    assert_eq!(refused.stderr(), "error: A human has control of app t1. Run `agent-terminal wait --control`.\n");
    let history = at.run(&["--session", "app", "--json", "history"]).success().json();
    assert_eq!(history["total"], 0);
}

#[test]
fn attach_needs_a_terminal() {
    let at = AgentTerminal::new();
    open_app(&at);
    let refused = at.run(&["--session", "app", "attach"]);
    assert_eq!(refused.code(), 1);
    assert_eq!(
        refused.stderr(),
        "error: attach shows the tab in your terminal, but stdin or stdout is not a terminal; read the screen with `agent-terminal snapshot` instead\n"
    );
}
