use crate::support::{AgentTerminal, Screen, Shown, fixture};

fn open_tui(at: &AgentTerminal) {
    at.run(&["open", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui")]).success();
}

#[test]
fn input_snapshot_rows_prints_only_that_range() {
    let at = AgentTerminal::new();
    open_tui(&at);
    insta::assert_snapshot!(at.run(&["press", "Enter", "--snapshot", "--rows", "2:3"]).success().screen(), @r"
    pressed 1 key in default t1
    default t1 · fixture-tui · 50x8 · alt-screen · cursor hidden · frame N
    2││alpha.txt         ││Selected: alpha.txt         │
    3││beta.txt          ││Size: 50x8                  │
    ");
}

#[test]
fn input_snapshot_i_lists_refs_for_what_is_on_screen() {
    let at = AgentTerminal::new();
    open_tui(&at);
    insta::assert_snapshot!(at.run(&["type", "z", "--snapshot", "-i"]).success().screen(), @r#"
    typed 1 key into default t1
    default t1 · fixture-tui · 50x8 · alt-screen · cursor hidden · frame N
    1│┌Files─────────────┐┌Details─────────────────────┐
    2││alpha.txt         ││Selected: alpha.txt         │
    3││beta.txt          ││Size: 50x8                  │
    4││gamma.txt         ││                            │
    5││delta.txt         ││                            │
    6││                  ││                            │
    7│└──────────────────┘└────────────────────────────┘
    8│Move: j/k | Quit: q | Crash: x
    refs:
      @t1 [panel]    "Files"     rows 1-7
      @t2 [panel]    "Details"   rows 1-7
      @t3 [selected] "alpha.txt" row 2 col 2
      @t4 [hint]     "Move: j/k" row 8 col 1
      @t5 [hint]     "Quit: q"   row 8 col 13
      @t6 [hint]     "Crash: x"  row 8 col 23
    "#);
}

#[test]
fn input_snapshot_squeeze_blank_folds_blank_runs() {
    let at = AgentTerminal::new();
    let _screen = Screen::open(&at, &Shown { size: "30x8", bytes: "top\n\n\n\nbottom", wait: "bottom" });
    let typed = at.run(&["type", "z", "--wait", "bottomz", "--snapshot", "-s"]).success();
    insta::assert_snapshot!(typed.screen(), @r"
    typed 1 key into default t1
    default t1 · sh · 30x8 · cursor 5,8 · frame N
    1│top
    …│ (rows 2-4 blank)
    5│bottomz
    …│ (rows 6-8 blank)
    ");
}

#[test]
fn a_snapshot_view_option_without_snapshot_is_a_usage_error() {
    let at = AgentTerminal::new();
    for view in [&["--rows", "2:3"][..], &["-i"], &["-s"], &["--ansi"], &["--scrollback"]] {
        let mut args = vec!["press", "Enter"];
        args.extend_from_slice(view);
        let run = at.run(&args);
        assert_eq!(run.code(), 1, "{view:?}: {}", run.stderr());
        assert!(run.stderr().contains("--snapshot"), "{view:?}: {}", run.stderr());
    }
}
