use crate::support::{AgentTerminal, fixture};

#[test]
fn resize_reflows_the_app_through_sigwinch() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "50x8", "--wait", "Size: 50x8", &fixture("fixture-tui")]).success();

    let resized = at.run(&["resize", "60x10", "--wait", "Size: 60x10"]).success();
    insta::assert_snapshot!(resized.screen(), @r"
    work app · fixture-tui · 60x10 · alt-screen · cursor hidden · frame N
     1│┌Files─────────────────┐┌Details───────────────────────────┐
     2││alpha.txt             ││Selected: alpha.txt               │
     3││beta.txt              ││Size: 60x10                       │
     4││gamma.txt             ││                                  │
     5││delta.txt             ││                                  │
     6││                      ││                                  │
     7││                      ││                                  │
     8││                      ││                                  │
     9│└──────────────────────┘└──────────────────────────────────┘
    10│Move: j/k | Quit: q | Crash: x
    ");
    assert_eq!(at.run(&["tab"]).success().stdout(), "work app · fixture-tui · 60x10 · running · current\n");
}

#[test]
fn resize_reaches_a_tab_other_than_the_current_one() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "50x8", "--wait", "Size: 50x8", &fixture("fixture-tui")]).success();
    at.run(&[
        "tab",
        "new",
        "--name",
        "api",
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

    let resized = at.run(&["--json", "--tab", "app", "resize", "40x6", "--wait", "Size: 40x6"]).success().json();
    assert_eq!(
        (resized["tab"].as_str(), resized["cols"].as_u64(), resized["rows"].as_u64()),
        (Some("app"), Some(40), Some(6))
    );
    assert_eq!(
        at.run(&["tab"]).success().stdout(),
        "work app · fixture-tui · 40x6 · running\n\
         work api · line-printer · 20x2 · running · current\n"
    );
}

#[test]
fn resizing_a_tab_whose_program_exited_fails_with_exit_code_5() {
    let at = AgentTerminal::new();
    at.run(&[
        "open",
        "--name",
        "app",
        "--size",
        "20x3",
        "--no-wait",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--exit",
        "7",
    ])
    .success();
    at.run(&["wait", "--exit"]).success();

    let run = at.run(&["resize", "30x5"]);
    assert_eq!(run.code(), 5);
    assert_eq!(
        run.stderr(),
        "error: work app: line-printer has exited with code 7; nothing more can be sent to it (read its last screen with `agent-terminal snapshot`, run it again with `agent-terminal restart`)\n"
    );
    assert_eq!(at.run(&["tab"]).success().stdout(), "work app · line-printer · 20x3 · exited 7 · current\n");
}

#[test]
fn resize_waits_for_a_stable_screen_when_no_wait_value_is_given() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "50x8", "--wait", "Size: 50x8", &fixture("fixture-tui")]).success();
    let resized = at.run(&["resize", "60x10", "--wait-stable"]).success();
    assert!(resized.stdout().starts_with("work app · fixture-tui · 60x10 · "), "{}", resized.stdout());
}

#[test]
fn a_size_out_of_range_with_json_is_a_json_error_on_stdout() {
    let at = AgentTerminal::new();
    let run = at.run(&["resize", "0x0", "--json"]);
    assert_eq!((run.code(), run.stderr().as_str()), (1, ""));
    assert_eq!(
        run.json(),
        serde_json::json!({ "error": {
            "code": 1,
            "message": "invalid value '0x0' for '<COLSxROWS>': columns and rows must be between 2 and 1000\n\nFor more information, try '--help'."
        } })
    );
}
