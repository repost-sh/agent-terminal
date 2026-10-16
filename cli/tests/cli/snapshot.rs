use std::process::Stdio;

use crate::support::{AgentTerminal, Run, fixture};

#[test]
fn snapshot_rows_shows_only_that_range() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "40x10", "--wait", "line 5", &fixture("line-printer"), "--lines", "5", "--hold"])
        .success();
    insta::assert_snapshot!(at.run(&["snapshot", "--rows", "2:3"]).success().screen(), @r"
    default t1 · line-printer · 40x10 · cursor 6,1 · frame N
     2│line 2
     3│line 3
    ");
    insta::assert_snapshot!(at.run(&["snapshot", "--rows", "4:8"]).success().screen(), @r"
    default t1 · line-printer · 40x10 · cursor 6,1 · frame N
     4│line 4
     5│line 5
     …│ (rows 6-8 blank)
    ");
    insta::assert_snapshot!(at.run(&["snapshot", "--rows", "9:20"]).success().screen(), @r"
    default t1 · line-printer · 40x10 · cursor 6,1 · frame N
     …│ (rows 9-10 blank)
    ");

    let json = at.run(&["--json", "snapshot", "--rows", "2:3"]).success().json();
    assert_eq!(json["first_row"], 2);
    assert_eq!(json["lines"], serde_json::json!(["line 2", "line 3"]));
}

#[test]
fn snapshot_into_a_pipe_its_reader_closed_ends_cleanly() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "40x10", "--wait", "line 5", &fixture("line-printer"), "--lines", "5", "--hold"])
        .success();
    let (reader, writer) = std::io::pipe().expect("a pipe");
    drop(reader);
    let snapshot =
        at.command(&["snapshot"]).stdout(writer).stderr(Stdio::piped()).output().expect("run agent-terminal");
    let run = Run(snapshot);
    assert_eq!(run.code(), 0, "stderr:\n{}", run.stderr());
    assert_eq!(run.stderr(), "");
}

#[test]
fn snapshot_rows_outside_the_screen_fails() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "40x10", "--wait", "line 1", &fixture("line-printer"), "--lines", "1", "--hold"])
        .success();
    let run = at.run(&["snapshot", "--rows", "11:12"]);
    assert_eq!(run.code(), 1);
    assert_eq!(run.stderr(), "error: rows 11:12 are outside the 40x10 screen\n");
}

#[test]
fn snapshot_scrollback_adds_lines_that_scrolled_off_the_top() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "30x5", "--wait", "line 8", &fixture("line-printer"), "--lines", "8", "--hold"])
        .success();
    insta::assert_snapshot!(at.run(&["snapshot", "--scrollback", "2"]).success().screen(), @r"
    default t1 · line-printer · 30x5 · cursor 5,1 · frame N
    -2│line 3
    -1│line 4
     1│line 5
     2│line 6
     3│line 7
     4│line 8
     …│ (row 5 blank)
    ");
    insta::assert_snapshot!(at.run(&["snapshot", "--scrollback"]).success().screen(), @r"
    default t1 · line-printer · 30x5 · cursor 5,1 · frame N
    -4│line 1
    -3│line 2
    -2│line 3
    -1│line 4
     1│line 5
     2│line 6
     3│line 7
     4│line 8
     …│ (row 5 blank)
    ");

    let json = at.run(&["--json", "snapshot", "--scrollback"]).success().json();
    assert_eq!(json["scrollback"], serde_json::json!(["line 1", "line 2", "line 3", "line 4"]));
    assert_eq!(json["lines"], serde_json::json!(["line 5", "line 6", "line 7", "line 8", ""]));
    assert!(at.run(&["--json", "snapshot"]).success().json().get("scrollback").is_none());
}

#[test]
fn snapshot_ansi_keeps_colors_and_styles() {
    let at = AgentTerminal::new();
    at.run(&[
        "open",
        "--size",
        "30x3",
        "--wait",
        "ready",
        "sh",
        "-c",
        "printf '\\033[1;31mError\\033[0m: \\033[7mok\\033[0m \\033[38;5;208;48;2;0;0;255mready\\033[0m'; read _",
    ])
    .success();
    assert_eq!(
        at.run(&["snapshot", "--ansi"]).success().screen(),
        "default t1 · sh · 30x3 · cursor 1,16 · frame N\n\
         1│\x1b[0;1;31mError\x1b[0m: \x1b[0;7mok\x1b[0m \x1b[0;38;5;208;48;2;0;0;255mready\x1b[0m\n\
         …│ (rows 2-3 blank)\n"
    );
    assert_eq!(
        at.run(&["snapshot"]).success().screen(),
        "default t1 · sh · 30x3 · cursor 1,16 · frame N\n1│Error: ok ready\n…│ (rows 2-3 blank)\n"
    );
}
