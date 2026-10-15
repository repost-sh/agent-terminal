use crate::support::{AgentTerminal, Cue};

#[test]
fn output_prints_everything_the_program_printed() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    cue.open_line_printer(&at, &[]);
    assert_eq!(at.run(&["output"]).success().stdout(), "line 1\nline 2\n");
    assert_eq!(at.run(&["--json", "output"]).success().json(), serde_json::json!({ "output": "line 1\nline 2\n" }));
}

#[test]
fn output_since_a_mark_prints_only_what_came_after_it() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    cue.open_line_printer(&at, &[]);
    let frame = at.run(&["get", "frame"]).success().stdout();
    assert_eq!(at.run(&["mark", "before-build"]).success().stdout(), format!("marked before-build at frame {frame}"));
    cue.print("compiling\nfinished\n");
    assert_eq!(at.run(&["output", "--since", "before-build"]).success().stdout(), "compiling\nfinished\n");

    at.run(&["mark", "before-build"]).success();
    cue.print("again\n");
    assert_eq!(at.run(&["output", "--since", "before-build"]).success().stdout(), "again\n");
    assert_eq!(
        at.run(&["--json", "mark", "after"]).success().json(),
        serde_json::json!({ "mark": "after", "frame": at.run(&["--json", "get", "frame"]).success().json()["frame"] })
    );
    assert_eq!(at.run(&["output", "--since", "after"]).success().stdout(), "");
}

#[test]
fn output_since_a_frame_prints_what_came_after_that_frame() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    cue.open_line_printer(&at, &[]);
    let frame = at.run(&["get", "frame"]).success().stdout().trim().to_string();
    cue.print("after the frame\n");
    assert_eq!(at.run(&["output", "--since", &frame]).success().stdout(), "after the frame\n");
}

#[test]
fn output_raw_keeps_escape_codes() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    cue.open_line_printer(&at, &[]);
    at.run(&["mark", "colors"]).success();
    cue.print("\x1b[31mred\x1b[0m\tdone\n");
    assert_eq!(at.run(&["output", "--since", "colors"]).success().stdout(), "red\tdone\n");
    assert_eq!(
        at.run(&["output", "--since", "colors", "--raw"]).success().stdout(),
        "\x1b[31mred\x1b[0m\tdone\r\n\x1b[6n"
    );
}

#[test]
fn output_older_than_the_kept_four_mib_fails_and_says_so() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    cue.open_line_printer(&at, &[]);
    at.run(&["mark", "start"]).success();
    cue.print(&format!("{}\n", "0123456789abcdef".repeat(4)).repeat(70_000));
    at.run(&["mark", "end"]).success();
    cue.print("tail\n");
    let run = at.run(&["output", "--since", "start"]);
    assert_eq!(run.code(), 1);
    assert_eq!(
        run.stderr(),
        "error: the output since start is no longer kept (agent-terminal keeps the last 4 MiB); mark a later point\n"
    );
    assert_eq!(at.run(&["output", "--since", "end"]).success().stdout(), "tail\n");
    assert_eq!(
        at.run(&["output", "--since", "1"]).stderr(),
        "error: the output since frame 1 is no longer kept (agent-terminal keeps the last 4 MiB); mark a later point\n"
    );
    assert_eq!(
        at.run(&["output"]).stderr(),
        "error: the output since the start is no longer kept (agent-terminal keeps the last 4 MiB); mark a later point\n"
    );
}

#[test]
fn output_since_an_unknown_mark_fails() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    cue.open_line_printer(&at, &[]);
    let run = at.run(&["output", "--since", "nope"]);
    assert_eq!(run.code(), 1);
    assert_eq!(run.stderr(), "error: no mark named \"nope\"; name a point with `agent-terminal mark nope`\n");
}
