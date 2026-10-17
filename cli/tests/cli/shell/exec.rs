use std::path::Path;

use super::tab::{ShellTab, timeless};

in_every_shell!(
    exec_prints_the_output_and_the_exit_code,
    wait_command_waits_for_a_long_running_command,
    a_cancelled_command_line_leaves_the_shell_ready,
    exec_of_a_line_the_shell_waits_to_finish_says_no_command_started,
    exec_after_a_line_the_shell_refused_starts_on_an_empty_command_line,
);

fn exec_prints_the_output_and_the_exit_code(shell: &'static str) {
    let tab = ShellTab::opened(shell);
    let cwd = tab.cwd();
    let run = tab.run(&["exec", "echo hello"]).success();
    assert_eq!(timeless(&run.stdout()), format!("hello\n1 · exit 0 · <time> · {cwd} · echo hello\n"));
}

fn wait_command_waits_for_a_long_running_command(shell: &'static str) {
    let tab = ShellTab::opened(shell);
    let cwd = tab.cwd();
    let hold = tab.hold();
    let at_prompt = tab.run(&["wait", "--command"]).success().stdout();
    assert_eq!(waited_n_ms(&at_prompt), "waited N ms for the shell command to finish in work shell\n");

    let line = "cat hold; echo finished";
    a_running_command_outlasts_exec_and_wait(&tab, &cwd, line);
    an_exec_behind_a_running_command_does_not_run(&tab);
    releasing_the_command_ends_the_wait(&tab, &hold, &cwd, line);
    a_typed_command_is_waited_for_too(&tab, &hold, &cwd);
}

fn waited_n_ms(text: &str) -> String {
    let waited = regex::Regex::new(r"^waited \d+ ms ").unwrap();
    waited.replace(text, "waited N ms ").into_owned()
}

fn a_running_command_outlasts_exec_and_wait(tab: &ShellTab, cwd: &str, line: &str) {
    let cut = tab.run(&["exec", line, "--timeout", "300"]);
    assert_eq!(cut.code(), 3);
    assert_eq!(
        cut.stderr().lines().next(),
        Some(
            "error: timed out after 300 ms waiting for `cat hold; echo finished` to finish in work shell; it is still running as command 1 (wait for it with `agent-terminal wait --command`, read its output with `agent-terminal history show 1`, interrupt it with `agent-terminal signal INT`)"
        )
    );
    assert_eq!(timeless(&tab.run(&["history"]).success().stdout()), format!("1 · running · <time> · {cwd} · {line}\n"));
    assert_eq!(tab.run(&["wait", "--command", "--timeout", "300"]).code(), 3);
}

fn an_exec_behind_a_running_command_does_not_run(tab: &ShellTab) {
    let queued = tab.run(&["exec", "echo next", "--timeout", "300"]);
    assert_eq!(queued.code(), 3);
    assert_eq!(
        queued.stderr().lines().next(),
        Some(
            "error: timed out after 300 ms waiting for `cat hold; echo finished` to finish in work shell, so `echo next` did not run (read the screen with `agent-terminal snapshot`, interrupt a command with `agent-terminal signal INT`)"
        )
    );
}

fn releasing_the_command_ends_the_wait(tab: &ShellTab, hold: &Path, cwd: &str, line: &str) {
    std::fs::write(hold, "released\n").expect("release the command");
    let finished = tab.run(&["wait", "--command"]).success().stdout();
    assert_eq!(waited_n_ms(&finished), "waited N ms for the shell command to finish in work shell (exit code 0)\n");
    assert_eq!(
        timeless(&tab.run(&["history", "show", "1"]).success().stdout()),
        format!("released\nfinished\n1 · exit 0 · <time> · {cwd} · {line}\n")
    );
}

fn a_typed_command_is_waited_for_too(tab: &ShellTab, hold: &Path, cwd: &str) {
    tab.run(&["type", "cat hold\n"]).success();
    assert_eq!(tab.run(&["wait", "--command", "--timeout", "300"]).code(), 3);
    std::fs::write(hold, "typed\n").expect("release the typed command");
    tab.run(&["wait", "--command"]).success();
    assert_eq!(
        timeless(&tab.run(&["history", "show", "2"]).success().stdout()),
        format!("typed\n2 · exit 0 · <time> · {cwd} · cat hold\n")
    );
}

fn exec_of_a_line_the_shell_waits_to_finish_says_no_command_started(shell: &'static str) {
    let tab = ShellTab::opened(shell);
    let cwd = tab.cwd();
    let unfinished = tab.run(&["exec", "echo 'unfinished", "--timeout", "300"]);
    assert_eq!(unfinished.code(), 3);
    assert_eq!(
        unfinished.stderr().lines().next(),
        Some(
            "error: timed out after 300 ms waiting for `echo 'unfinished` to finish in work shell; the shell has not started a command for it, so the line may be unfinished (read the screen with `agent-terminal snapshot`, cancel the line with `agent-terminal press Ctrl+c`)"
        )
    );
    assert_eq!(tab.run(&["history"]).success().stdout(), "no commands have run in work shell\n");
    tab.run(&["press", "Ctrl+c"]).success();
    tab.run(&["wait", "--command"]).success();
    assert_eq!(
        timeless(&tab.run(&["exec", "echo after"]).success().stdout()),
        format!("after\n1 · exit 0 · <time> · {cwd} · echo after\n")
    );
}

fn a_cancelled_command_line_leaves_the_shell_ready(shell: &'static str) {
    let tab = ShellTab::opened(shell);
    let cwd = tab.cwd();
    for cancel in [&["press", "Ctrl+c"][..], &["signal", "INT"]] {
        tab.run(&["type", "echo 'unfinished\n"]).success();
        assert_eq!(tab.run(&["wait", "--command", "--timeout", "300"]).code(), 3, "{cancel:?}");
        tab.run(cancel).success();
        tab.run(&["wait", "--command"]).success();
    }
    assert_eq!(
        timeless(&tab.run(&["exec", "echo after"]).success().stdout()),
        format!("after\n1 · exit 0 · <time> · {cwd} · echo after\n")
    );
}

fn exec_after_a_line_the_shell_refused_starts_on_an_empty_command_line(shell: &'static str) {
    let tab = ShellTab::opened(shell);
    let cwd = tab.cwd();
    let refused = tab.run(&["exec", ")"]);
    assert_eq!(refused.code(), 1);
    assert_eq!(
        refused.stderr().lines().next(),
        Some("error: the shell in work shell ran no command for `)`; the screen shows why")
    );
    assert_eq!(
        timeless(&tab.run(&["exec", "echo after"]).success().stdout()),
        format!("after\n1 · exit 0 · <time> · {cwd} · echo after\n")
    );
}

#[test]
fn exec_says_when_the_shell_exits() {
    let tab = ShellTab::opened("zsh");
    let run = tab.run(&["exec", "exit 4"]);
    assert_eq!(run.code(), 5);
    assert_eq!(
        run.stderr().lines().next(),
        Some("error: stopped waiting for the shell command to finish: zsh in work shell has exited with code 4")
    );
    assert_eq!(tab.run(&["wait", "--command"]).code(), 5);
    assert_eq!(tab.run(&["exec", "echo late"]).code(), 5);
}

#[test]
fn exec_refuses_what_is_not_one_command_line() {
    let tab = ShellTab::opened("zsh");
    an_empty_or_multi_line_command_is_refused(&tab);
    let unparsed = tab.run(&["exec", ")"]);
    assert_eq!(unparsed.code(), 1);
    let stderr = unparsed.stderr();
    assert_eq!(
        stderr.lines().next(),
        Some("error: the shell in work shell ran no command for `)`; the screen shows why")
    );
    assert!(stderr.contains("parse error"), "{stderr}");
    assert_eq!(tab.run(&["history"]).success().stdout(), "no commands have run in work shell\n");
}

fn an_empty_or_multi_line_command_is_refused(tab: &ShellTab) {
    let empty = tab.run(&["exec", " "]);
    assert_eq!(empty.code(), 1);
    assert_eq!(empty.stderr(), "error: exec needs a command line, e.g. exec \"cargo test\"\n");
    let lines = tab.run(&["exec", "echo a\necho b"]);
    assert_eq!(lines.code(), 1);
    assert_eq!(lines.stderr(), "error: exec runs one command line; join commands with ; or && instead of newlines\n");
}

#[test]
fn the_activity_log_keeps_the_exit_code_exec_check_exits_with() {
    let tab = ShellTab::opened("zsh");
    assert_eq!(tab.run(&["exec", "--check", "sh -c 'exit 7'"]).code(), 7);
    tab.run(&["exec", "false"]).success();
    let log = std::fs::read_to_string(tab.at.home().join("sessions/work/activity.jsonl")).expect("activity log");
    let exits: Vec<(String, i64)> = log
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("activity entry"))
        .map(|entry| (entry["command"].as_str().unwrap().to_string(), entry["exit"].as_i64().unwrap()))
        .skip(1)
        .collect();
    assert_eq!(exits, [("exec --check \"sh -c 'exit 7'\"".to_string(), 7), ("exec false".to_string(), 0)]);
}

#[test]
fn execs_sent_together_run_one_after_the_other() {
    let tab = ShellTab::opened("zsh");
    let words = ["alpha", "bravo", "charlie", "delta"];
    let outputs: Vec<String> = std::thread::scope(|scope| {
        let calls: Vec<_> = words
            .iter()
            .map(|word| {
                let line = format!("echo {word}");
                let tab = &tab;
                scope.spawn(move || tab.run(&["exec", &line]).success().stdout())
            })
            .collect();
        calls.into_iter().map(|call| call.join().unwrap()).collect()
    });
    let mut ids = Vec::new();
    for (word, output) in words.iter().zip(&outputs) {
        let (printed, record) = output.split_once('\n').expect("output, then the record line");
        assert_eq!(printed, *word, "{output}");
        assert!(record.ends_with(&format!(" · echo {word}\n")), "{output}");
        ids.push(record.split(' ').next().unwrap().to_string());
    }
    ids.sort();
    assert_eq!(ids, ["1", "2", "3", "4"]);
    assert_eq!(tab.run(&["history"]).success().stdout().lines().count(), 4);
}
