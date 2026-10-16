use crate::support::{AgentTerminal, fixture};

const NAME_THE_TAB: &str = "error: name the tab after the program or app it runs, e.g. `--name api`; the name is how later calls reach it (`--tab api`)\n";

#[test]
fn open_tab_new_and_shell_without_a_name_fail_with_how_to_name_the_tab_and_start_nothing() {
    let at = AgentTerminal::new();
    for call in [&["open", "--size", "20x2", "cat"][..], &["tab", "new", "cat"], &["shell", "--shell", "bash"]] {
        let refused = at.run(call);
        let shown = (refused.code(), refused.stdout(), refused.stderr());
        assert_eq!(shown, (1, String::new(), NAME_THE_TAB.to_string()), "{call:?}");
    }
    let named_badly = at.run(&["open", "--name", "My App", "cat"]);
    assert_eq!(named_badly.code(), 1);
    assert!(named_badly.stderr().contains("\"My App\" is not a name"), "{}", named_badly.stderr());
    assert_eq!(at.run(&["session", "list"]).success().stdout(), "work · no tabs\n");
}

fn open_two_tabs(at: &AgentTerminal) {
    let printer = fixture("line-printer");
    at.run(&["open", "--name", "app", "--size", "20x2", "--wait", "line 1", &printer, "--lines", "1", "--hold"])
        .success();
    at.run(&["tab", "new", "--name", "api", "--size", "20x3", "--wait", "line 2", &printer, "--lines", "2", "--hold"])
        .success();
}

#[test]
fn tab_new_runs_another_terminal_in_the_same_session() {
    let at = AgentTerminal::new();
    let printer = fixture("line-printer");
    at.run(&["open", "--name", "app", "--size", "20x2", "--wait", "line 1", &printer, "--lines", "1", "--hold"])
        .success();
    let opened = at
        .run(&["tab", "new", "--name", "api", "--size", "20x3", "--wait", "line 2", &printer, "--lines", "2", "--hold"])
        .success();
    insta::assert_snapshot!(opened.screen(), @r"
    work api · line-printer · 20x3 · cursor 3,1 · frame N
    1│line 1
    2│line 2
    …│ (row 3 blank)
    ");

    let listed = "work app · line-printer · 20x2 · running\n\
                  work api · line-printer · 20x3 · running · current\n";
    assert_eq!(at.run(&["tab"]).success().stdout(), listed);
    assert_eq!(at.run(&["tab", "list"]).success().stdout(), listed);
    let cwd = std::env::current_dir().unwrap().to_string_lossy().into_owned();
    assert_eq!(
        at.run(&["--json", "tab"]).success().json(),
        serde_json::json!({ "session": "work", "current": "api", "tabs": [
            {
                "tab": "app", "program": "line-printer", "command": format!("{printer} --lines 1 --hold"), "cwd": cwd,
                "cols": 20, "rows": 2, "exited": false, "exit_code": null, "control": "agent",
            },
            {
                "tab": "api", "program": "line-printer", "command": format!("{printer} --lines 2 --hold"), "cwd": cwd,
                "cols": 20, "rows": 3, "exited": false, "exit_code": null, "control": "agent",
            },
        ] })
    );
}

#[test]
fn a_name_another_tab_of_the_session_holds_is_refused() {
    let at = AgentTerminal::new();
    open_two_tabs(&at);

    let again = at.run(&["tab", "new", "--name", "app", "--size", "20x2", &fixture("line-printer"), "--lines", "1"]);
    assert_eq!(
        (again.code(), again.stderr()),
        (1, "error: tab app already runs line-printer in session work; pick another name\n".to_string())
    );
    assert_eq!(
        at.run(&["tab"]).success().stdout(),
        "work app · line-printer · 20x2 · running\n\
         work api · line-printer · 20x3 · running · current\n"
    );
}

#[test]
fn tab_with_a_name_switches_the_current_tab() {
    let at = AgentTerminal::new();
    open_two_tabs(&at);

    assert_eq!(
        at.run(&["tab", "app"]).success().stdout(),
        "work app · line-printer · 20x2 · running · current\n\
         work api · line-printer · 20x3 · running\n"
    );
    insta::assert_snapshot!(at.run(&["snapshot"]).success().screen(), @r"
    work app · line-printer · 20x2 · cursor 2,1 · frame N
    1│line 1
    …│ (row 2 blank)
    ");

    at.run(&["tab", "api"]).success();
    assert!(at.run(&["snapshot"]).success().stdout().starts_with("work api · "));

    assert_eq!(at.run(&["tab", "app", "--json"]).success().json()["current"], "app");
}

#[test]
fn the_tab_option_targets_a_tab_by_name_without_switching_to_it() {
    let at = AgentTerminal::new();
    open_two_tabs(&at);

    insta::assert_snapshot!(at.run(&["--tab", "app", "snapshot"]).success().screen(), @r"
    work app · line-printer · 20x2 · cursor 2,1 · frame N
    1│line 1
    …│ (row 2 blank)
    ");
    assert!(at.run(&["snapshot"]).success().stdout().starts_with("work api · "));
    assert_eq!(at.run(&["--json", "tab"]).success().json()["current"], "api");
}

#[test]
fn a_tab_is_reached_by_its_name_only_so_a_number_like_t1_is_no_tab() {
    let at = AgentTerminal::new();
    open_two_tabs(&at);

    let calls = [
        (&["tab", "t1"][..], "t1"),
        (&["--tab", "t1", "snapshot"], "t1"),
        (&["tab", "close", "t1"], "t1"),
        (&["tab", "2"], "2"),
    ];
    for (call, tab) in calls {
        let run = at.run(call);
        assert_eq!(
            (run.code(), run.stderr()),
            (1, format!("error: no tab {tab} in session work; list tabs with `agent-terminal tab`\n")),
            "{call:?}"
        );
    }
    a_tab_of_another_session_or_a_name_against_the_rule_is_an_error(&at);
}

fn a_tab_of_another_session_or_a_name_against_the_rule_is_an_error(at: &AgentTerminal) {
    let other_session = at.run(&["--session", "other", "--tab", "app", "snapshot"]);
    assert_eq!(other_session.code(), 1);
    assert_eq!(other_session.stderr(), "error: no session other; create it with `agent-terminal session new other`\n");

    for against in [&["tab", "App"][..], &["--tab", "App", "snapshot"], &["tab", "close", "App"]] {
        let run = at.run(against);
        assert_eq!(run.code(), 1, "{against:?}");
        assert!(
            run.stderr().contains("\"App\" is not a name: a name is 1 to 24 characters of a-z, 0-9 and -"),
            "{}",
            run.stderr()
        );
    }
}

#[test]
fn tab_rename_gives_the_current_or_the_named_tab_a_new_name() {
    let at = AgentTerminal::new();
    open_two_tabs(&at);

    assert_eq!(
        at.run(&["tab", "rename", "web"]).success().stdout(),
        "work app · line-printer · 20x2 · running\n\
         work web · line-printer · 20x3 · running · current\n"
    );
    insta::assert_snapshot!(at.run(&["--tab", "web", "snapshot"]).success().screen(), @r"
    work web · line-printer · 20x3 · cursor 3,1 · frame N
    1│line 1
    2│line 2
    …│ (row 3 blank)
    ");
    at.run(&["--tab", "app", "tab", "rename", "db"]).success();
    assert_eq!(at.run(&["--json", "tab"]).success().json()["tabs"][0]["tab"], "db");
    a_rename_to_a_name_that_is_taken_or_against_the_rule_is_refused(&at);
}

fn a_rename_to_a_name_that_is_taken_or_against_the_rule_is_refused(at: &AgentTerminal) {
    let taken = at.run(&["tab", "rename", "db"]);
    assert_eq!(
        (taken.code(), taken.stderr()),
        (1, "error: tab db already runs line-printer in session work; pick another name\n".to_string())
    );
    let against = at.run(&["tab", "rename", "Web"]);
    assert_eq!(against.code(), 1);
    assert!(against.stderr().contains("\"Web\" is not a name"), "{}", against.stderr());
    let missing = at.run(&["--tab", "app", "tab", "rename", "cache"]);
    assert_eq!(
        (missing.code(), missing.stderr()),
        (1, "error: no tab app in session work; list tabs with `agent-terminal tab`\n".to_string())
    );
}

#[test]
fn reading_commands_reach_a_tab_other_than_the_current_one() {
    let at = AgentTerminal::new();
    open_two_tabs(&at);

    assert_eq!(at.run(&["--tab", "app", "get", "text"]).success().stdout(), "line 1\n");
    assert_eq!(at.run(&["--tab", "app", "get", "size"]).success().stdout(), "20x2\n");
    assert_eq!(at.run(&["--tab", "app", "find", "line 2"]).code(), 1);
    assert_eq!(at.run(&["find", "line 2"]).success().stdout(), "2,1 line 2\n");
    marks_and_diffs_reach_a_tab_other_than_the_current_one(&at);
}

fn marks_and_diffs_reach_a_tab_other_than_the_current_one(at: &AgentTerminal) {
    at.run(&["--tab", "app", "mark", "before"]).success();
    assert_eq!(at.run(&["--tab", "app", "output", "--since", "before"]).success().stdout(), "");
    assert_eq!(at.run(&["output", "--since", "before"]).code(), 1);
    at.run(&["--tab", "app", "snapshot"]).success();
    let diff = at.run(&["--tab", "app", "diff", "snapshot"]).success().stdout();
    assert!(diff.ends_with(" · no rows changed\n"), "{diff}");
    assert_eq!(at.run(&["--json", "tab"]).success().json()["current"], "api");
}

#[test]
fn input_reaches_a_tab_other_than_the_current_one() {
    let printer = fixture("line-printer");
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "echo", "--size", "60x4", "--wait", "ready", &fixture("input-echo")]).success();
    at.run(&[
        "tab", "new", "--name", "printer", "--size", "20x2", "--wait", "line 1", &printer, "--lines", "1", "--hold",
    ])
    .success();

    let typed = at.run(&["--tab", "echo", "type", "abc", "--wait", "abc"]).success();
    assert_eq!(typed.stdout(), "typed 3 keys into work echo\n");
    assert_eq!(at.run(&["--json", "--tab", "echo", "snapshot"]).success().json()["lines"][1], "abc");
    assert_eq!(at.run(&["--json", "tab"]).success().json()["current"], "printer");
}

#[test]
fn tab_close_closes_a_tab_by_name_or_the_current_one() {
    let printer = fixture("line-printer");
    let at = AgentTerminal::new();
    open_two_tabs(&at);
    at.run(&["tab", "new", "--name", "web", "--size", "20x2", "--wait", "line 1", &printer, "--lines", "1", "--hold"])
        .success();

    assert_eq!(at.run(&["tab", "close", "app"]).success().stdout(), "closed work app\n");
    assert_eq!(
        at.run(&["tab"]).success().stdout(),
        "work api · line-printer · 20x3 · running\n\
         work web · line-printer · 20x2 · running · current\n"
    );
    closing_without_a_name_closes_the_current_tab(&at);
}

fn closing_without_a_name_closes_the_current_tab(at: &AgentTerminal) {
    assert_eq!(at.run(&["tab", "close"]).success().stdout(), "closed work web\n");
    assert_eq!(at.run(&["tab"]).success().stdout(), "work api · line-printer · 20x3 · running · current\n");

    assert_eq!(at.run(&["--tab", "api", "close"]).success().stdout(), "closed work api\n");
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
