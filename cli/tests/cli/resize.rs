use crate::support::{AgentTerminal, fixture};

#[test]
fn resize_reflows_the_app_through_sigwinch() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "50x8", "--wait", "Size: 50x8", &fixture("fixture-tui")]).success();

    let resized = at.run(&["resize", "60x10", "--wait", "Size: 60x10"]).success();
    insta::assert_snapshot!(resized.screen(), @r"
    default t1 · fixture-tui · 60x10 · alt-screen · cursor hidden · frame N
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
    assert_eq!(at.run(&["tab"]).success().stdout(), "default t1 · fixture-tui · 60x10 · running · current\n");
}

#[test]
fn resize_reaches_a_tab_other_than_the_current_one() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "50x8", "--wait", "Size: 50x8", &fixture("fixture-tui")]).success();
    at.run(&["tab", "new", "--size", "20x2", "--wait", "line 1", &fixture("line-printer"), "--lines", "1", "--hold"])
        .success();

    let resized = at.run(&["--json", "--tab", "t1", "resize", "40x6", "--wait", "Size: 40x6"]).success().json();
    assert_eq!(
        (resized["tab"].as_str(), resized["cols"].as_u64(), resized["rows"].as_u64()),
        (Some("t1"), Some(40), Some(6))
    );
    assert_eq!(
        at.run(&["tab"]).success().stdout(),
        "default t1 · fixture-tui · 40x6 · running\n\
         default t2 · line-printer · 20x2 · running · current\n"
    );
}

#[test]
fn resizing_a_tab_whose_program_exited_fails_with_exit_code_5() {
    let at = AgentTerminal::new();
    at.run(&[
        "open",
        "--size",
        "20x3",
        "--wait-stable",
        "5000",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--exit",
        "7",
    ])
    .success();

    let run = at.run(&["resize", "30x5"]);
    assert_eq!(run.code(), 5);
    assert_eq!(
        run.stderr(),
        "error: default t1: line-printer has exited with code 7; nothing more can be sent to it (read its last screen with `agent-terminal snapshot`, run it again with `agent-terminal restart`)\n"
    );
    assert_eq!(at.run(&["tab"]).success().stdout(), "default t1 · line-printer · 20x3 · exited 7 · current\n");
}

#[test]
fn resize_waits_for_a_stable_screen_when_no_wait_value_is_given() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "50x8", "--wait", "Size: 50x8", &fixture("fixture-tui")]).success();
    let resized = at.run(&["resize", "60x10", "--wait-stable"]).success();
    assert!(resized.stdout().starts_with("default t1 · fixture-tui · 60x10 · "), "{}", resized.stdout());
}
