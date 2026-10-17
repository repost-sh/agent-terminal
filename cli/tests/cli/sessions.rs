use crate::support::{AgentTerminal, fixture};

#[test]
fn session_new_creates_an_empty_session_and_a_second_one_with_its_name_fails_naming_the_holder() {
    let at = AgentTerminal::without_session();
    assert_eq!(at.run(&["session", "new", "a"]).success().stdout(), "created session a\n");
    assert_eq!(at.run(&["session", "list"]).success().stdout(), "a · no tabs\n");
    at.run(&["--session", "a", "open", "--name", "app", "--size", "20x2", "cat"]).success();
    let taken = at.run(&["session", "new", "a"]);
    let stderr = taken.stderr();
    assert_eq!(taken.code(), 1);
    assert!(
        stderr.starts_with("error: session a is taken: open for ")
            && stderr.ends_with(", 1 tab: app; pick another name for this conversation\n"),
        "{stderr}"
    );
}

#[test]
fn session_new_refuses_a_name_outside_the_rule() {
    let unnamed = AgentTerminal::without_session().run(&["session", "new", "Not_A_Name"]);
    assert_eq!(unnamed.code(), 1);
    assert!(unnamed.stderr().contains("is not a name: a name is 1 to 24 characters"), "{}", unnamed.stderr());
}

#[test]
fn session_id_is_an_unknown_subcommand() {
    let at = AgentTerminal::without_session();
    let id = at.run(&["session", "id"]);
    assert_eq!(id.code(), 1);
    assert!(id.stderr().contains("unrecognized subcommand 'id'"), "{}", id.stderr());
}

#[test]
fn session_prints_the_current_session_name() {
    let at = AgentTerminal::new();
    assert_eq!(at.run(&["session"]).success().stdout(), "work\n");
    assert_eq!(at.run(&["--session", "qa", "session"]).success().stdout(), "qa\n");
    assert_eq!(at.run_with_session_env("from-env", &["session"]).success().stdout(), "from-env\n");
    assert_eq!(at.run_with_session_env("from-env", &["--session", "flag", "session"]).success().stdout(), "flag\n");
    assert_eq!(
        at.run(&["--json", "--session", "qa", "session"]).success().json(),
        serde_json::json!({ "session": "qa" })
    );
}

#[test]
fn session_without_a_session_says_how_to_name_and_create_one() {
    let unnamed = AgentTerminal::without_session().run(&["session"]);
    assert_eq!(unnamed.code(), 1);
    assert!(
        unnamed.stderr().starts_with("error: name this conversation's session after its task"),
        "{}",
        unnamed.stderr()
    );
}

#[test]
fn sessions_never_see_each_others_terminals() {
    let printer = fixture("line-printer");
    let at = AgentTerminal::new();
    at.run(&["session", "new", "a"]).success();
    at.run(&["session", "new", "b"]).success();
    at.run(&[
        "--session",
        "a",
        "open",
        "--name",
        "app",
        "--size",
        "20x2",
        "--wait",
        "line 1",
        &printer,
        "--lines",
        "1",
        "--hold",
    ])
    .success();
    at.run(&["--session", "a", "wait", "--cursor", "2,1"]).success();
    let open_in_b = ["open", "--name", "app", "--size", "20x3", "--wait", "line 2", &printer, "--lines", "2", "--hold"];
    at.run_with_session_env("b", &open_in_b).success();
    at.run_with_session_env("b", &["wait", "--cursor", "3,1"]).success();

    let default = at.run(&["snapshot"]);
    assert_eq!(default.code(), 1);
    assert!(default.stderr().contains("no terminal is open in session work"), "{}", default.stderr());

    insta::assert_snapshot!(at.run(&["--session", "a", "snapshot"]).success().screen(), @r"
    a app · line-printer · 20x2 · cursor 2,1 · frame N
    1│line 1
    …│ (row 2 blank)
    ");
    insta::assert_snapshot!(at.run_with_session_env("b", &["snapshot"]).success().screen(), @r"
    b app · line-printer · 20x3 · cursor 3,1 · frame N
    1│line 1
    2│line 2
    …│ (row 3 blank)
    ");
}

#[test]
fn session_list_shows_every_tab_with_its_program_size_and_status() {
    let at = AgentTerminal::without_session();
    open_a_with_a_running_and_an_exited_tab_and_b(&at);

    assert_eq!(
        at.run(&["session", "list"]).success().stdout(),
        "a app · line-printer · 30x5 · running\n\
         a api · line-printer · 20x3 · exited 4 · current\n\
         b app · fixture-tui · 50x8 · running · current\n"
    );
    session_list_json_describes_every_tab(&at);
}

fn open_a_with_a_running_and_an_exited_tab_and_b(at: &AgentTerminal) {
    let tui = fixture("fixture-tui");
    let printer = fixture("line-printer");
    at.run(&["session", "new", "a"]).success();
    at.run(&["session", "new", "b"]).success();
    at.run(&[
        "--session",
        "a",
        "open",
        "--name",
        "app",
        "--size",
        "30x5",
        "--wait",
        "line 1",
        &printer,
        "--lines",
        "1",
        "--hold",
    ])
    .success();
    at.run(&[
        "--session",
        "a",
        "open",
        "--name",
        "api",
        "--size",
        "20x3",
        "--no-wait",
        &printer,
        "--lines",
        "1",
        "--exit",
        "4",
    ])
    .success();
    at.run(&["--session", "a", "wait", "--exit"]).success();
    at.run(&["--session", "b", "open", "--name", "app", "--size", "50x8", "--wait", "Quit: q", &tui]).success();
}

fn session_list_json_describes_every_tab(at: &AgentTerminal) {
    let printer = fixture("line-printer");
    let cwd = std::env::current_dir().unwrap().to_string_lossy().into_owned();
    assert_eq!(
        at.run(&["--json", "session", "list"]).success().json(),
        serde_json::json!({ "sessions": [
            { "session": "a", "current": "api", "tabs": [
                {
                    "tab": "app", "program": "line-printer", "command": format!("{printer} --lines 1 --hold"), "cwd": cwd,
                    "cols": 30, "rows": 5, "exited": false, "exit_code": null, "control": "agent",
                },
                {
                    "tab": "api", "program": "line-printer", "command": format!("{printer} --lines 1 --exit 4"), "cwd": cwd,
                    "cols": 20, "rows": 3, "exited": true, "exit_code": 4, "control": "agent",
                },
            ] },
            { "session": "b", "current": "app", "tabs": [
                {
                    "tab": "app", "program": "fixture-tui", "command": fixture("fixture-tui"), "cwd": cwd,
                    "cols": 50, "rows": 8, "exited": false, "exit_code": null, "control": "agent",
                },
            ] },
        ] })
    );
}

#[test]
fn session_list_with_nothing_open_says_so_without_starting_the_daemon() {
    let at = AgentTerminal::without_session();
    assert_eq!(at.run(&["session", "list"]).success().stdout(), "no sessions are open\n");
    assert_eq!(at.run(&["--json", "session", "list"]).success().json(), serde_json::json!({ "sessions": [] }));
    assert!(!at.home().join("daemon.sock").exists(), "session list must not start the daemon");
}
