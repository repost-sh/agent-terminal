use crate::support::{AgentTerminal, fixture};

#[test]
fn session_prints_the_current_session_name() {
    let at = AgentTerminal::new();
    assert_eq!(at.run(&["session"]).success().stdout(), "default\n");
    assert_eq!(at.run(&["--session", "qa", "session"]).success().stdout(), "qa\n");
    assert_eq!(at.run_with_session_env("from-env", &["session"]).success().stdout(), "from-env\n");
    assert_eq!(at.run_with_session_env("from-env", &["--session", "flag", "session"]).success().stdout(), "flag\n");
    assert_eq!(
        at.run(&["--json", "--session", "qa", "session"]).success().json(),
        serde_json::json!({ "session": "qa" })
    );
}

#[test]
fn sessions_never_see_each_others_terminals() {
    let at = AgentTerminal::new();
    at.run(&[
        "--session",
        "a",
        "open",
        "--size",
        "20x2",
        "--wait",
        "line 1",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--hold",
    ])
    .success();
    at.run_with_session_env(
        "b",
        &["open", "--size", "20x3", "--wait", "line 2", &fixture("line-printer"), "--lines", "2", "--hold"],
    )
    .success();

    let default = at.run(&["snapshot"]);
    assert_eq!(default.code(), 1);
    assert!(default.stderr().contains("no terminal is open in session default"), "{}", default.stderr());

    insta::assert_snapshot!(at.run(&["--session", "a", "snapshot"]).success().screen(), @r"
    a t1 · line-printer · 20x2 · cursor 2,1 · frame N
    1│line 1
    …│ (row 2 blank)
    ");
    insta::assert_snapshot!(at.run_with_session_env("b", &["snapshot"]).success().screen(), @r"
    b t1 · line-printer · 20x3 · cursor 3,1 · frame N
    1│line 1
    2│line 2
    …│ (row 3 blank)
    ");
}

#[test]
fn session_list_shows_every_tab_with_its_program_size_and_status() {
    let at = AgentTerminal::new();
    at.run(&[
        "--session",
        "a",
        "open",
        "--size",
        "30x5",
        "--wait",
        "line 1",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--hold",
    ])
    .success();
    at.run(&[
        "--session",
        "a",
        "open",
        "--size",
        "20x3",
        "--wait-stable",
        "5000",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--exit",
        "4",
    ])
    .success();
    at.run(&["--session", "b", "open", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui")]).success();

    assert_eq!(
        at.run(&["session", "list"]).success().stdout(),
        "a t1 · line-printer · 30x5 · running\n\
         a t2 · line-printer · 20x3 · exited 4 · current\n\
         b t1 · fixture-tui · 50x8 · running · current\n"
    );
    assert_eq!(
        at.run(&["--json", "session", "list"]).success().json(),
        serde_json::json!({ "sessions": [
            { "session": "a", "current": "t2", "tabs": [
                { "tab": "t1", "program": "line-printer", "cols": 30, "rows": 5, "exited": false, "exit_code": null, "control": "agent" },
                { "tab": "t2", "program": "line-printer", "cols": 20, "rows": 3, "exited": true, "exit_code": 4, "control": "agent" },
            ] },
            { "session": "b", "current": "t1", "tabs": [
                { "tab": "t1", "program": "fixture-tui", "cols": 50, "rows": 8, "exited": false, "exit_code": null, "control": "agent" },
            ] },
        ] })
    );
}

#[test]
fn session_list_with_nothing_open_says_so_without_starting_the_daemon() {
    let at = AgentTerminal::new();
    assert_eq!(at.run(&["session", "list"]).success().stdout(), "no sessions are open\n");
    assert_eq!(at.run(&["--json", "session", "list"]).success().json(), serde_json::json!({ "sessions": [] }));
    assert!(!at.home().join("daemon.sock").exists(), "session list must not start the daemon");
}

#[test]
fn session_id_is_stable_for_a_checkout_and_unique_across_worktrees() {
    let at = AgentTerminal::new();
    let main = at.home().join("main");
    let linked = at.home().join("linked");
    std::fs::create_dir_all(main.join(".git")).unwrap();
    std::fs::create_dir_all(main.join("src/deep")).unwrap();
    std::fs::create_dir_all(&linked).unwrap();
    std::fs::write(linked.join(".git"), "gitdir: ../main/.git/worktrees/linked\n").unwrap();
    let id = |dir: &std::path::Path| {
        at.run_in(dir, &["session", "id", "--scope", "worktree", "--prefix", "qa"]).success().stdout()
    };

    let from_root = id(&main);
    assert_eq!(id(&main.join("src/deep")), from_root);
    assert_eq!(id(&main), from_root);
    assert_ne!(id(&linked), from_root);
    let hash = from_root.strip_prefix("qa-").and_then(|rest| rest.strip_suffix('\n')).expect("qa-<hash>");
    assert!(hash.len() == 12 && hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "{hash}");
    assert!(!at.home().join("daemon.sock").exists(), "session id must not start the daemon");
}

#[test]
fn session_id_matches_agent_browser_for_the_same_folder() {
    let at = AgentTerminal::new();
    let root = std::path::Path::new("/");
    assert_eq!(at.run_in(root, &["session", "id", "--prefix", "demo"]).success().stdout(), "demo-8a5edab28263\n");
    assert_eq!(at.run_in(root, &["session", "id", "--scope", "worktree"]).success().stdout(), "8a5edab28263\n");
    assert_eq!(
        at.run_in(root, &["--json", "session", "id", "--scope", "worktree", "--prefix", "demo"]).success().json(),
        serde_json::json!({ "session": "demo-8a5edab28263", "scope": "worktree", "path": "/", "hash": "8a5edab28263" })
    );
}
