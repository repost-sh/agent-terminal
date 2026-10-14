use crate::support::{AgentTerminal, fixture};

fn open_two_tabs(at: &AgentTerminal) {
    at.run(&["open", "--size", "20x2", "--wait", "line 1", &fixture("line-printer"), "--lines", "1", "--hold"])
        .success();
    at.run(&["tab", "new", "--size", "20x3", "--wait", "line 2", &fixture("line-printer"), "--lines", "2", "--hold"])
        .success();
}

#[test]
fn tab_new_runs_another_terminal_in_the_same_session() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x2", "--wait", "line 1", &fixture("line-printer"), "--lines", "1", "--hold"])
        .success();
    let opened = at
        .run(&["tab", "new", "--size", "20x3", "--wait", "line 2", &fixture("line-printer"), "--lines", "2", "--hold"])
        .success();
    insta::assert_snapshot!(opened.screen(), @r"
    default t2 · line-printer · 20x3 · cursor 3,1 · frame N
    1│line 1
    2│line 2
    …│ (row 3 blank)
    ");

    let listed = "default t1 · line-printer · 20x2 · running\n\
                  default t2 · line-printer · 20x3 · running · current\n";
    assert_eq!(at.run(&["tab"]).success().stdout(), listed);
    assert_eq!(at.run(&["tab", "list"]).success().stdout(), listed);
    assert_eq!(
        at.run(&["--json", "tab"]).success().json(),
        serde_json::json!({ "session": "default", "current": "t2", "tabs": [
            { "tab": "t1", "program": "line-printer", "cols": 20, "rows": 2, "exited": false, "exit_code": null },
            { "tab": "t2", "program": "line-printer", "cols": 20, "rows": 3, "exited": false, "exit_code": null },
        ] })
    );
}

#[test]
fn tab_with_an_id_switches_the_current_tab() {
    let at = AgentTerminal::new();
    open_two_tabs(&at);

    assert_eq!(
        at.run(&["tab", "t1"]).success().stdout(),
        "default t1 · line-printer · 20x2 · running · current\n\
         default t2 · line-printer · 20x3 · running\n"
    );
    insta::assert_snapshot!(at.run(&["snapshot"]).success().screen(), @r"
    default t1 · line-printer · 20x2 · cursor 2,1 · frame N
    1│line 1
    …│ (row 2 blank)
    ");

    at.run(&["tab", "2"]).success();
    assert!(at.run(&["snapshot"]).success().stdout().starts_with("default t2 · "));

    assert_eq!(at.run(&["tab", "t1", "--json"]).success().json()["current"], "t1");
}

#[test]
fn the_tab_option_targets_a_tab_without_switching_to_it() {
    let at = AgentTerminal::new();
    open_two_tabs(&at);

    insta::assert_snapshot!(at.run(&["--tab", "t1", "snapshot"]).success().screen(), @r"
    default t1 · line-printer · 20x2 · cursor 2,1 · frame N
    1│line 1
    …│ (row 2 blank)
    ");
    assert!(at.run(&["snapshot"]).success().stdout().starts_with("default t2 · "));
    assert_eq!(at.run(&["--json", "tab"]).success().json()["current"], "t2");
}

#[test]
fn a_tab_that_does_not_exist_is_an_error() {
    let at = AgentTerminal::new();
    open_two_tabs(&at);

    let snapshot = at.run(&["--tab", "t9", "snapshot"]);
    assert_eq!(snapshot.code(), 1);
    assert_eq!(snapshot.stderr(), "error: no tab t9 in session default; list tabs with `agent-terminal tab`\n");

    let switch = at.run(&["tab", "t9"]);
    assert_eq!(switch.code(), 1);
    assert_eq!(switch.stderr(), "error: no tab t9 in session default; list tabs with `agent-terminal tab`\n");

    let other_session = at.run(&["--session", "other", "--tab", "t1", "snapshot"]);
    assert_eq!(other_session.code(), 1);
    assert_eq!(other_session.stderr(), "error: no tab t1 in session other; list tabs with `agent-terminal tab`\n");

    for malformed in [&["tab", "first"][..], &["--tab", "first", "snapshot"], &["tab", "close", "first"]] {
        let run = at.run(malformed);
        assert_ne!(run.code(), 0, "{malformed:?}");
        assert!(run.stderr().contains("invalid tab id: first (expected t1, t2, ...)"), "{}", run.stderr());
    }
}

#[test]
fn tab_close_closes_a_tab_by_id_or_the_current_one() {
    let at = AgentTerminal::new();
    open_two_tabs(&at);
    at.run(&["tab", "new", "--size", "20x2", "--wait", "line 1", &fixture("line-printer"), "--lines", "1", "--hold"])
        .success();

    assert_eq!(at.run(&["tab", "close", "t1"]).success().stdout(), "closed default t1\n");
    assert_eq!(
        at.run(&["tab"]).success().stdout(),
        "default t2 · line-printer · 20x3 · running\n\
         default t3 · line-printer · 20x2 · running · current\n"
    );

    assert_eq!(at.run(&["tab", "close"]).success().stdout(), "closed default t3\n");
    assert_eq!(at.run(&["tab"]).success().stdout(), "default t2 · line-printer · 20x3 · running · current\n");

    assert_eq!(at.run(&["--tab", "t2", "close"]).success().stdout(), "closed default t2\n");
    assert_eq!(at.run(&["tab"]).success().stdout(), "no tabs are open in session default\n");
}

#[test]
fn tab_with_nothing_open_says_so_without_starting_the_daemon() {
    let at = AgentTerminal::new();
    assert_eq!(at.run(&["--session", "qa", "tab"]).success().stdout(), "no tabs are open in session qa\n");
    assert_eq!(
        at.run(&["--json", "--session", "qa", "tab"]).success().json(),
        serde_json::json!({ "session": "qa", "current": null, "tabs": [] })
    );
    assert!(!at.home().join("daemon.sock").exists(), "tab must not start the daemon");
}
