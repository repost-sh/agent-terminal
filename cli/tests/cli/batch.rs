use crate::support::{AgentTerminal, fixture};

#[test]
fn batch_runs_commands_in_order_and_prints_each_output_and_exit_code() {
    let at = AgentTerminal::new();
    let run = at.run(&["batch", "open --size 20x3 --wait hi sh -c 'echo hi; read _'", "snapshot", "close"]).success();
    insta::assert_snapshot!(run.screen(), @r"
    [1] open --size 20x3 --wait hi sh -c 'echo hi; read _' · exit 0
    default t1 · sh · 20x3 · cursor 2,1 · frame N
    1│hi
    …│ (rows 2-3 blank)

    [2] snapshot · exit 0
    default t1 · sh · 20x3 · cursor 2,1 · frame N
    1│hi
    …│ (rows 2-3 blank)

    [3] close · exit 0
    closed default t1
    ");
}

#[test]
fn options_of_open_after_the_program_are_open_options_in_a_batch_too() {
    let at = AgentTerminal::new();
    let run =
        at.run(&["batch", "open sh -c 'echo \"args: $#\"; read _' sh --size 30x3 --wait args", "close"]).success();
    insta::assert_snapshot!(run.screen(), @r#"
    [1] open sh -c 'echo "args: $#"; read _' sh --size 30x3 --wait args · exit 0
    default t1 · sh · 30x3 · cursor 2,1 · frame N
    1│args: 0
    …│ (rows 2-3 blank)

    [2] close · exit 0
    closed default t1
    "#);
}

#[test]
fn a_failed_command_does_not_stop_the_batch_and_sets_its_exit_code() {
    let at = AgentTerminal::new();
    let run = at.run(&["batch", "snapshot", "open --size 20x2 cat", "close", "close"]);
    assert_eq!(run.code(), 1);
    insta::assert_snapshot!(run.screen(), @r"
    [1] snapshot · exit 1
    error: no terminal is open in session default; start one with `agent-terminal open <program>`

    [2] open --size 20x2 cat · exit 0
    default t1 · cat · 20x2 · cursor 1,1 · frame N
    …│ (rows 1-2 blank)

    [3] close · exit 0
    closed default t1

    [4] close · exit 1
    error: no terminal is open in session default; start one with `agent-terminal open <program>`
    ");
}

#[test]
fn bail_stops_at_the_first_failure() {
    let at = AgentTerminal::new();
    let run = at.run(&["batch", "--bail", "open --size 20x2 cat", "close", "close", "open --size 20x2 cat"]);
    assert_eq!(run.code(), 1);
    insta::assert_snapshot!(run.screen(), @r"
    [1] open --size 20x2 cat · exit 0
    default t1 · cat · 20x2 · cursor 1,1 · frame N
    …│ (rows 1-2 blank)

    [2] close · exit 0
    closed default t1

    [3] close · exit 1
    error: no terminal is open in session default; start one with `agent-terminal open <program>`

    [4] open --size 20x2 cat · not run
    ");
    assert_eq!(at.run(&["snapshot"]).code(), 1, "the command after the failure must not have run");
}

#[test]
fn commands_take_the_session_and_timeout_of_the_batch_unless_they_set_their_own() {
    let at = AgentTerminal::new();
    let run = at.run(&[
        "--session",
        "qa",
        "--timeout",
        "300",
        "batch",
        "open --size 20x2 cat",
        "open --size 20x2 --wait never cat",
        "snapshot --session other",
        "close --session qa",
    ]);
    assert_eq!(run.code(), 3);
    insta::assert_snapshot!(run.screen(), @r#"
    [1] open --size 20x2 cat · exit 0
    qa t1 · cat · 20x2 · cursor 1,1 · frame N
    …│ (rows 1-2 blank)

    [2] open --size 20x2 --wait never cat · exit 3
    error: timed out after 300 ms waiting for "never" to appear; qa t2 is still running (read it with `agent-terminal snapshot`, stop it with `agent-terminal close`)
    qa t2 · cat · 20x2 · cursor 1,1 · frame N
    …│ (rows 1-2 blank)

    [3] snapshot --session other · exit 1
    error: no terminal is open in session other; start one with `agent-terminal open <program>`

    [4] close --session qa · exit 0
    closed qa t2
    "#);
}

#[test]
fn commands_act_on_the_tab_of_the_batch_unless_they_name_their_own() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x2", "--wait", "one", "sh", "-c", "echo one; read _"]).success();
    at.run(&["open", "--size", "20x2", "cat"]).success();
    let run = at.run(&["--tab", "t1", "batch", "snapshot", "snapshot --tab t2"]).success();
    insta::assert_snapshot!(run.screen(), @r"
    [1] snapshot · exit 0
    default t1 · sh · 20x2 · cursor 2,1 · frame N
    1│one
    …│ (row 2 blank)

    [2] snapshot --tab t2 · exit 0
    default t2 · cat · 20x2 · cursor 1,1 · frame N
    …│ (rows 1-2 blank)
    ");
}

#[test]
fn the_session_variable_applies_to_the_batch_unless_the_batch_names_a_session() {
    let at = AgentTerminal::new();
    let from_env = [("AGENT_TERMINAL_SESSION", "from-env")];
    let run = at.run_with(&["batch", "open --size 20x2 cat", "close"], "", &from_env).success();
    assert!(run.stdout().ends_with("closed from-env t1\n"), "{}", run.stdout());
    let run = at.run_with(&["--session", "qa", "batch", "open --size 20x2 cat", "close"], "", &from_env).success();
    assert!(run.stdout().ends_with("closed qa t1\n"), "{}", run.stdout());
}

#[test]
fn batch_reads_a_json_array_of_commands_from_stdin() {
    let at = AgentTerminal::new();
    let stdin = r#"[["open", "--size", "20x2", "--wait", "a b", "sh", "-c", "echo \"a b\"; read _"], "close"]"#;
    let run = at.run_with(&["batch"], stdin, &[]).success();
    insta::assert_snapshot!(run.screen(), @r#"
    [1] open --size 20x2 --wait 'a b' sh -c 'echo "a b"; read _' · exit 0
    default t1 · sh · 20x2 · cursor 2,1 · frame N
    1│a b
    …│ (row 2 blank)

    [2] close · exit 0
    closed default t1
    "#);
}

#[test]
fn batch_without_commands_says_how_to_pass_them() {
    let at = AgentTerminal::new();
    let needs_commands =
        "error: batch needs commands: pass each one as a quoted argument, or pipe a JSON array to stdin\n";
    for stdin in ["", " \n", "[]"] {
        let run = at.run_with(&["batch"], stdin, &[]);
        assert_eq!((run.code(), run.stderr().as_str()), (1, needs_commands), "stdin {stdin:?}");
    }

    let run = at.run_with(&["batch"], "open htop", &[]);
    assert_eq!(run.code(), 1);
    assert!(
        run.stderr().starts_with(
            "error: stdin is not a JSON array of commands, like [[\"open\", \"htop\"], \"snapshot\"]: expected value"
        ),
        "{}",
        run.stderr()
    );
}

#[test]
fn a_command_that_is_not_valid_stops_the_batch_before_anything_runs() {
    let at = AgentTerminal::new();
    an_unknown_command_stops_the_batch(&at);
    a_command_that_runs_on_its_own_stops_the_batch(&at);
    an_unclosed_quote_stops_the_batch(&at);
    assert_eq!(at.run(&["snapshot"]).code(), 1, "no command of a batch that is not valid may run");
}

fn an_unknown_command_stops_the_batch(at: &AgentTerminal) {
    let run = at.run(&["batch", "open --size 20x2 cat", "nonsense --flag"]);
    assert_eq!((run.code(), run.stdout().as_str()), (1, ""));
    insta::assert_snapshot!(run.stderr(), @r"
    error: batch command 2 is not valid, so nothing ran: nonsense --flag
    error: unrecognized subcommand 'nonsense'

    Usage: agent-terminal [OPTIONS] <COMMAND>

    For more information, try '--help'.
    ");
}

fn a_command_that_runs_on_its_own_stops_the_batch(at: &AgentTerminal) {
    let run = at.run(&["batch", "open --size 20x2 cat", "version"]);
    assert_eq!((run.code(), run.stdout().as_str()), (1, ""));
    assert_eq!(
        run.stderr(),
        "error: batch command 2 is not valid, so nothing ran: version\nthis command runs on its own, not inside batch\n"
    );
}

fn an_unclosed_quote_stops_the_batch(at: &AgentTerminal) {
    let run = at.run(&["batch", "open --size 20x2 cat", "open sh -c 'echo"]);
    assert_eq!((run.code(), run.stdout().as_str()), (1, ""));
    assert_eq!(
        run.stderr(),
        "error: batch command 2 is not valid, so nothing ran: open sh -c 'echo\na quote or escape is not closed\n"
    );
}

#[test]
fn json_output_lists_each_command_with_its_exit_code_and_result() {
    let at = AgentTerminal::new();
    let run = at.run(&["--json", "batch", "--bail", "open --size 20x2 cat", "snapshot --session other", "close"]);
    assert_eq!(run.code(), 1);
    let entries = run.json();
    assert_eq!(entries.as_array().map(Vec::len), Some(3), "{entries}");
    the_first_entry_carries_its_args_exit_code_and_result(&entries[0]);
    assert_eq!(
        entries[1],
        serde_json::json!({
            "args": ["snapshot", "--session", "other"],
            "exit_code": 1,
            "error": {
                "code": 1,
                "message": "no terminal is open in session other; start one with `agent-terminal open <program>`"
            }
        })
    );
    assert_eq!(entries[2], serde_json::json!({ "args": ["close"], "exit_code": null }));
}

fn the_first_entry_carries_its_args_exit_code_and_result(entry: &serde_json::Value) {
    assert_eq!(entry["args"], serde_json::json!(["open", "--size", "20x2", "cat"]));
    assert_eq!(entry["exit_code"], 0);
    assert_eq!((entry["result"]["session"].as_str(), entry["result"]["tab"].as_str()), (Some("default"), Some("t1")));
    assert_eq!(entry["result"]["lines"], serde_json::json!(["", ""]));
}

#[test]
fn batch_runs_any_command_the_daemon_serves() {
    let at = AgentTerminal::new();
    let stdin = serde_json::json!([
        ["open", "--size", "60x3", "--wait", "ready", fixture("input-echo")],
        ["type", "hi", "--wait", "hi"],
        ["mark", "typed"],
        ["close"]
    ])
    .to_string();
    let out = at.run_with(&["batch"], &stdin, &[]).success().screen();
    let blocks: Vec<&str> = out.split("\n\n").collect();
    assert_eq!(
        blocks[1..],
        [
            "[2] type hi --wait hi · exit 0\ntyped 2 keys into default t1",
            "[3] mark typed · exit 0\nmarked typed at frame N",
            "[4] close · exit 0\nclosed default t1\n"
        ],
        "{out}"
    );
}

#[test]
fn stopping_the_daemon_cannot_be_part_of_a_batch() {
    let at = AgentTerminal::new();
    let run = at.run(&["batch", "open --size 20x2 cat", "daemon stop", "close"]);
    assert_eq!(run.code(), 1);
    insta::assert_snapshot!(run.screen(), @r"
    [1] open --size 20x2 cat · exit 0
    default t1 · cat · 20x2 · cursor 1,1 · frame N
    …│ (rows 1-2 blank)

    [2] daemon stop · exit 1
    error: stopping the daemon cannot be part of a batch

    [3] close · exit 0
    closed default t1
    ");
}

#[test]
fn batch_in_a_terminal_without_commands_does_not_wait_for_input() {
    let at = AgentTerminal::new();
    let run = at
        .run(&["open", "--size", "100x3", "--wait", "pipe a JSON", env!("CARGO_BIN_EXE_agent-terminal"), "batch"])
        .success();
    assert!(
        run.stdout().contains(
            "1│error: batch needs commands: pass each one as a quoted argument, or pipe a JSON array to stdin\n"
        ),
        "{}",
        run.stdout()
    );
}

#[test]
fn a_batch_command_that_is_not_valid_with_json_is_a_json_error_on_stdout() {
    let at = AgentTerminal::new();
    let run = at.run(&["batch", "--json", "get nonsense"]);
    assert_eq!((run.code(), run.stderr().as_str()), (1, ""));
    assert_eq!(
        run.json(),
        serde_json::json!({ "error": {
            "code": 1,
            "message": "batch command 1 is not valid, so nothing ran: get nonsense\nerror: unrecognized subcommand 'nonsense'\n\nUsage: get [OPTIONS] <COMMAND>\n\nFor more information, try '--help'."
        } })
    );
}
