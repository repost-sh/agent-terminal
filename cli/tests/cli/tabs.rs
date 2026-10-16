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
    work t2 · line-printer · 20x3 · cursor 3,1 · frame N
    1│line 1
    2│line 2
    …│ (row 3 blank)
    ");

    let listed = "work t1 · line-printer · 20x2 · running\n\
                  work t2 · line-printer · 20x3 · running · current\n";
    assert_eq!(at.run(&["tab"]).success().stdout(), listed);
    assert_eq!(at.run(&["tab", "list"]).success().stdout(), listed);
    let printer = fixture("line-printer");
    let cwd = std::env::current_dir().unwrap().to_string_lossy().into_owned();
    assert_eq!(
        at.run(&["--json", "tab"]).success().json(),
        serde_json::json!({ "session": "work", "current": "t2", "tabs": [
            {
                "tab": "t1", "program": "line-printer", "command": format!("{printer} --lines 1 --hold"), "cwd": cwd,
                "cols": 20, "rows": 2, "exited": false, "exit_code": null, "control": "agent",
            },
            {
                "tab": "t2", "program": "line-printer", "command": format!("{printer} --lines 2 --hold"), "cwd": cwd,
                "cols": 20, "rows": 3, "exited": false, "exit_code": null, "control": "agent",
            },
        ] })
    );
}

#[test]
fn tab_with_an_id_switches_the_current_tab() {
    let at = AgentTerminal::new();
    open_two_tabs(&at);

    assert_eq!(
        at.run(&["tab", "t1"]).success().stdout(),
        "work t1 · line-printer · 20x2 · running · current\n\
         work t2 · line-printer · 20x3 · running\n"
    );
    insta::assert_snapshot!(at.run(&["snapshot"]).success().screen(), @r"
    work t1 · line-printer · 20x2 · cursor 2,1 · frame N
    1│line 1
    …│ (row 2 blank)
    ");

    at.run(&["tab", "2"]).success();
    assert!(at.run(&["snapshot"]).success().stdout().starts_with("work t2 · "));

    assert_eq!(at.run(&["tab", "t1", "--json"]).success().json()["current"], "t1");
}

#[test]
fn the_tab_option_targets_a_tab_without_switching_to_it() {
    let at = AgentTerminal::new();
    open_two_tabs(&at);

    insta::assert_snapshot!(at.run(&["--tab", "t1", "snapshot"]).success().screen(), @r"
    work t1 · line-printer · 20x2 · cursor 2,1 · frame N
    1│line 1
    …│ (row 2 blank)
    ");
    assert!(at.run(&["snapshot"]).success().stdout().starts_with("work t2 · "));
    assert_eq!(at.run(&["--json", "tab"]).success().json()["current"], "t2");
}

#[test]
fn a_tab_that_does_not_exist_is_an_error() {
    let at = AgentTerminal::new();
    open_two_tabs(&at);

    let snapshot = at.run(&["--tab", "t9", "snapshot"]);
    assert_eq!(snapshot.code(), 1);
    assert_eq!(snapshot.stderr(), "error: no tab t9 in session work; list tabs with `agent-terminal tab`\n");

    let switch = at.run(&["tab", "t9"]);
    assert_eq!(switch.code(), 1);
    assert_eq!(switch.stderr(), "error: no tab t9 in session work; list tabs with `agent-terminal tab`\n");

    a_tab_of_another_session_or_a_malformed_id_is_an_error(&at);
}

fn a_tab_of_another_session_or_a_malformed_id_is_an_error(at: &AgentTerminal) {
    let other_session = at.run(&["--session", "other", "--tab", "t1", "snapshot"]);
    assert_eq!(other_session.code(), 1);
    assert_eq!(other_session.stderr(), "error: no session other; create it with `agent-terminal session new other`\n");

    for malformed in [&["tab", "first"][..], &["--tab", "first", "snapshot"], &["tab", "close", "first"]] {
        let run = at.run(malformed);
        assert_eq!(run.code(), 1, "{malformed:?}");
        assert!(run.stderr().contains("invalid tab id: first (expected t1, t2, ...)"), "{}", run.stderr());
    }
}

#[test]
fn reading_commands_reach_a_tab_other_than_the_current_one() {
    let at = AgentTerminal::new();
    open_two_tabs(&at);

    assert_eq!(at.run(&["--tab", "t1", "get", "text"]).success().stdout(), "line 1\n");
    assert_eq!(at.run(&["--tab", "t1", "get", "size"]).success().stdout(), "20x2\n");
    assert_eq!(at.run(&["--tab", "t1", "find", "line 2"]).code(), 1);
    assert_eq!(at.run(&["find", "line 2"]).success().stdout(), "2,1 line 2\n");
    marks_and_diffs_reach_a_tab_other_than_the_current_one(&at);
}

fn marks_and_diffs_reach_a_tab_other_than_the_current_one(at: &AgentTerminal) {
    at.run(&["--tab", "t1", "mark", "before"]).success();
    assert_eq!(at.run(&["--tab", "t1", "output", "--since", "before"]).success().stdout(), "");
    assert_eq!(at.run(&["output", "--since", "before"]).code(), 1);
    at.run(&["--tab", "t1", "snapshot"]).success();
    let diff = at.run(&["--tab", "t1", "diff", "snapshot"]).success().stdout();
    assert!(diff.ends_with(" · no rows changed\n"), "{diff}");
    assert_eq!(at.run(&["--json", "tab"]).success().json()["current"], "t2");
}

#[test]
fn input_reaches_a_tab_other_than_the_current_one() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "60x4", "--wait", "ready", &fixture("input-echo")]).success();
    at.run(&["tab", "new", "--size", "20x2", "--wait", "line 1", &fixture("line-printer"), "--lines", "1", "--hold"])
        .success();

    let typed = at.run(&["--tab", "t1", "type", "abc", "--wait", "abc"]).success();
    assert_eq!(typed.stdout(), "typed 3 keys into work t1\n");
    assert_eq!(at.run(&["--json", "--tab", "t1", "snapshot"]).success().json()["lines"][1], "abc");
    assert_eq!(at.run(&["--json", "tab"]).success().json()["current"], "t2");
}

#[test]
fn tab_close_closes_a_tab_by_id_or_the_current_one() {
    let at = AgentTerminal::new();
    open_two_tabs(&at);
    at.run(&["tab", "new", "--size", "20x2", "--wait", "line 1", &fixture("line-printer"), "--lines", "1", "--hold"])
        .success();

    assert_eq!(at.run(&["tab", "close", "t1"]).success().stdout(), "closed work t1\n");
    assert_eq!(
        at.run(&["tab"]).success().stdout(),
        "work t2 · line-printer · 20x3 · running\n\
         work t3 · line-printer · 20x2 · running · current\n"
    );
    closing_without_an_id_closes_the_current_tab(&at);
}

fn closing_without_an_id_closes_the_current_tab(at: &AgentTerminal) {
    assert_eq!(at.run(&["tab", "close"]).success().stdout(), "closed work t3\n");
    assert_eq!(at.run(&["tab"]).success().stdout(), "work t2 · line-printer · 20x3 · running · current\n");

    assert_eq!(at.run(&["--tab", "t2", "close"]).success().stdout(), "closed work t2\n");
    assert_eq!(at.run(&["tab"]).success().stdout(), "no tabs are open in session work\n");
}

#[test]
fn tab_in_a_new_session_says_it_has_none_and_in_an_unknown_one_says_to_create_it() {
    let at = AgentTerminal::new();
    assert_eq!(at.run(&["tab"]).success().stdout(), "no tabs are open in session work\n");
    assert_eq!(
        at.run(&["--json", "tab"]).success().json(),
        serde_json::json!({ "session": "work", "current": null, "tabs": [] })
    );
    let unknown = at.run(&["--session", "qa", "tab"]);
    assert_eq!(
        (unknown.code(), unknown.stderr()),
        (1, "error: no session qa; create it with `agent-terminal session new qa`\n".to_string())
    );
}
