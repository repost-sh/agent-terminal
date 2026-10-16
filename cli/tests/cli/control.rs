use crate::support::{AgentTerminal, Run, fixture};

const BINARY: &str = env!("CARGO_BIN_EXE_agent-terminal");

fn open_app(at: &AgentTerminal) {
    at.run(&["session", "new", "app"]).success();
    at.run(&["--session", "app", "open", "--size", "40x6", "--wait", "ready", &fixture("input-echo")]).success();
}

fn attach(at: &AgentTerminal, from: &str, extra: &[&str], ready: &str) -> Run {
    at.run(&["session", "new", from]).success();
    let mut args =
        vec!["--session", from, "open", "--size", "70x10", "--wait", ready, BINARY, "--", "--session", "app"];
    args.push("attach");
    args.extend_from_slice(extra);
    at.run(&args)
}

const REFUSED: &str = "A human has control of app t1. Run `agent-terminal wait --control`.";

fn detach(at: &AgentTerminal, from: &str) -> String {
    at.run(&["--session", from, "press", "Ctrl+p", "Ctrl+q"]).success();
    at.run(&["--session", from, "wait", "--exit"]).success();
    at.run(&["--session", from, "get", "text"]).success().stdout()
}

#[test]
fn while_a_human_is_attached_input_exits_4_and_wait_control_resumes_once_they_detach() {
    let at = AgentTerminal::new();
    open_app(&at);
    attach(&at, "human", &[], "ctrl-p ctrl-q detaches").success();

    input_is_refused_with_exit_4(&at);
    wait_control_runs_out_of_time_while_the_human_holds_it(&at);
    detaching_ends_the_agents_wait(&at);
    at.run(&["--session", "human", "wait", "--exit"]).success();
    let detached = at.run(&["--session", "human", "get", "text"]).success().stdout();
    assert!(detached.contains("detached from app t1"), "{detached}");
    at.run(&["--session", "app", "press", "j", "--wait", "j"]).success();
    assert_eq!(at.received("app"), "j");
    each_refusal_is_logged_with_its_tab_and_error(&at);
}

fn input_is_refused_with_exit_4(at: &AgentTerminal) {
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
}

fn wait_control_runs_out_of_time_while_the_human_holds_it(at: &AgentTerminal) {
    let held = at.run(&["--session", "app", "--timeout", "300", "wait", "--control"]);
    assert_eq!(held.code(), 3);
    assert_eq!(
        held.stderr(),
        "error: timed out after 300 ms waiting for a human to hand back control of app t1; wait longer with --timeout <ms>\n"
    );
    let asserted = at.run(&["--session", "app", "--timeout", "300", "wait", "--control", "--assert"]);
    assert_eq!(asserted.code(), 2);
    assert_eq!(asserted.stderr(), "error: a human did not hand back control of app t1 within 300 ms\n");
}

fn detaching_ends_the_agents_wait(at: &AgentTerminal) {
    std::thread::scope(|scope| {
        let waiting = scope.spawn(|| at.run(&["--session", "app", "--json", "wait", "--control"]));
        at.run(&["--session", "human", "press", "Ctrl+p", "Ctrl+q"]).success();
        let waited = waiting.join().expect("the waiting call").success().json();
        assert_eq!(waited["until"], "a human to hand back control");
        assert_eq!((&waited["session"], &waited["tab"]), (&serde_json::json!("app"), &serde_json::json!("t1")));
    });
}

fn each_refusal_is_logged_with_its_tab_and_error(at: &AgentTerminal) {
    let refusals: Vec<serde_json::Value> = at.activity("app").into_iter().filter(|entry| entry["exit"] == 4).collect();
    let commands: Vec<&serde_json::Value> = refusals.iter().map(|entry| &entry["command"]).collect();
    assert_eq!(commands, ["--session app press j", "--session app --json type x", "--session app signal INT"]);
    for refusal in &refusals {
        the_refusal_names_the_tab_and_the_human(refusal);
    }
}

fn the_refusal_names_the_tab_and_the_human(refusal: &serde_json::Value) {
    assert_eq!(refusal["tab"], "t1");
    assert_eq!(refusal["error"], REFUSED);
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
    at.run(&["session", "new", "app"]).success();
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
    let mut input = HumanInput::default();
    for entry in &entries {
        input.take(entry, &entries);
    }
    assert_eq!(input.typed, "hé");
    assert_eq!(input.pressed, keys);
    assert!(detach(&at, "human").contains("detached from app t1"));
}

const SENT: [&str; 24] = [
    r"\e[1;5A",
    r"\eOP",
    r"\e[3~",
    r"\e[24;2~",
    r"\e[99~",
    r"\e[Z",
    r"\ex",
    r"\0",
    r"\x1c",
    r"\x7f",
    r"\xff",
    r"\xc3\xa9",
    r"\e[1;3D",
    r"\eOH",
    r"\e[7~",
    r"\e[4~",
    r"\e[",
    "B",
    r"\eO",
    "Q",
    r"\e",
    r"\e[200~pasted text\e[201~",
    r"\e[200~two ",
    r"parts\e[201~",
];

#[test]
fn attach_turns_the_sequences_a_terminal_sends_into_keys_for_the_tab() {
    let at = AgentTerminal::new();
    at.run(&["session", "new", "app"]).success();
    at.run(&["--session", "app", "open", "--size", "160x4", "--wait", "ready", &fixture("input-echo")]).success();
    attach(&at, "human", &[], "ctrl-p ctrl-q detaches").success();
    for raw in SENT {
        at.run(&["--session", "human", "send", raw]).success();
    }
    let expected =
        r"\e[1;5A\eOP\e[3~\e[24;2~\e[Z\ex\x00\x1c\x7f\xc3\xa9\e[1;3D\e[H\e[H\e[F\e[B\eOQ\epasted texttwo parts";
    at.run(&["--session", "app", "wait", expected]).success();
    assert_eq!(at.received("app"), expected);
    assert!(detach(&at, "human").contains("detached from app t1"));
}

#[test]
fn attach_in_a_window_no_taller_than_the_tab_has_no_room_for_the_hint() {
    let at = AgentTerminal::new();
    open_app(&at);
    at.run(&["session", "new", "human"]).success();
    at.run(&[
        "--session",
        "human",
        "open",
        "--size",
        "40x6",
        "--wait",
        "ready",
        BINARY,
        "--",
        "--session",
        "app",
        "attach",
    ])
    .success();
    let shown = at.run(&["--session", "human", "get", "text"]).success().stdout();
    assert!(shown.starts_with("ready") && !shown.contains("detaches"), "{shown}");
    assert!(detach(&at, "human").contains("detached from app t1"));
}

#[test]
fn attaching_to_a_session_that_does_not_exist_says_so() {
    let at = AgentTerminal::new();
    at.run(&["session", "new", "human"]).success();
    at.run(&["--session", "human", "open", "--size", "70x6", BINARY, "--", "--session", "nowhere", "attach"]).success();
    at.run(&["--session", "human", "wait", "--exit"]).success();
    let shown = at.run(&["--session", "human", "get", "text"]).success().stdout();
    assert_eq!(shown, "error: no session nowhere; create it with `agent-terminal session new\nnowhere`\n");
}

#[derive(Default)]
struct HumanInput {
    typed: String,
    pressed: Vec<String>,
}

impl HumanInput {
    fn take(&mut self, entry: &serde_json::Value, entries: &[serde_json::Value]) {
        assert_eq!((&entry["tab"], &entry["exit"]), (&serde_json::json!("t1"), &serde_json::json!(0)), "{entry}");
        match (&entry["human"]["typed"], &entry["human"]["pressed"]) {
            (serde_json::Value::String(text), _) => self.typed_text(entry, entries, text),
            (_, serde_json::Value::String(key)) => self.pressed_key(entry, key),
            _ => panic!("a human entry types or presses: {entry}"),
        }
    }

    fn typed_text(&mut self, entry: &serde_json::Value, entries: &[serde_json::Value], text: &str) {
        assert!(entry["command"].as_str().unwrap().starts_with("type "), "{entry}");
        assert!(self.pressed.is_empty(), "typing came after a key: {entries:?}");
        self.typed.push_str(text);
    }

    fn pressed_key(&mut self, entry: &serde_json::Value, key: &str) {
        assert_eq!(entry["command"], format!("press {key}"), "{entry}");
        self.pressed.push(key.to_string());
    }
}

#[test]
fn attach_mirrors_the_tab_with_a_line_saying_how_to_detach() {
    let at = AgentTerminal::new();
    at.run(&["session", "new", "app"]).success();
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
    assert!(detach(&at, "watcher").contains("detached from app t1"));
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
    let home = crate::support::scratch("home-");
    at.run(&["session", "new", "app"]).success();
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
    assert!(detach(&at, "human").contains("detached from app t1"));
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
