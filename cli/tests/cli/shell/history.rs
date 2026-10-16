use super::tab::{ShellTab, timeless};

in_every_shell!(
    history_keeps_every_command_with_its_exit_code_cwd_and_output,
    a_command_the_users_history_leaves_out_is_recorded_as_typed,
);

fn history_keeps_every_command_with_its_exit_code_cwd_and_output(shell: &'static str) {
    let tab = ShellTab::opened(shell);
    let cwd = tab.cwd();
    let sub = format!("{cwd}/sub");
    history_says_when_nothing_ran_or_matched(&tab);
    tab.run(&["restart"]).success();

    let numbers = exec_prints_each_command_with_its_exit_code(&tab, &cwd, &sub);
    let all = format!(
        "2 · exit 1 · <time> · {cwd} · false\n\
         3 · exit 3 · <time> · {cwd} · sh -c 'echo to stderr >&2; exit 3'\n\
         4 · exit 0 · <time> · {cwd} · mkdir sub && cd sub\n\
         5 · exit 0 · <time> · {sub} · seq 1 25\n"
    );
    let lines: Vec<&str> = all.lines().collect();
    history_lists_and_filters_the_commands(&tab, &all, &lines);
    history_show_prints_one_command_with_its_output(&tab, &numbers, lines[3]);
    assert_eq!(tab.run(&["get", "cwd"]).success().stdout(), format!("{sub}\n"));
}

fn history_says_when_nothing_ran_or_matched(tab: &ShellTab) {
    assert_eq!(tab.run(&["history"]).success().stdout(), "no commands have run in work t1\n");
    tab.run(&["exec", "true"]).success();
    assert_eq!(tab.run(&["history", "--failed"]).success().stdout(), "no command matches in work t1 (1 in all)\n");
}

fn exec_prints_each_command_with_its_exit_code(tab: &ShellTab, cwd: &str, sub: &str) -> String {
    let failed = tab.run(&["exec", "false"]).success();
    assert_eq!(timeless(&failed.stdout()), format!("2 · exit 1 · <time> · {cwd} · false\n"));
    let checked = tab.run(&["exec", "--check", "sh -c 'echo to stderr >&2; exit 3'"]);
    assert_eq!(checked.code(), 3, "{}", checked.stderr());
    assert_eq!(
        timeless(&checked.stdout()),
        format!("to stderr\n3 · exit 3 · <time> · {cwd} · sh -c 'echo to stderr >&2; exit 3'\n")
    );
    tab.run(&["exec", "--check", "mkdir sub && cd sub"]).success();
    let numbers = format!("{}\n", (1..=25).map(|n| n.to_string()).collect::<Vec<_>>().join("\n"));
    let scrolled = tab.run(&["exec", "seq 1 25"]).success();
    assert_eq!(timeless(&scrolled.stdout()), format!("{numbers}5 · exit 0 · <time> · {sub} · seq 1 25\n"));
    numbers
}

fn history_lists_and_filters_the_commands(tab: &ShellTab, all: &str, lines: &[&str]) {
    assert_eq!(timeless(&tab.run(&["history"]).success().stdout()), all);
    assert_eq!(
        timeless(&tab.run(&["history", "--failed"]).success().stdout()),
        format!("{}\n{}\n", lines[0], lines[1])
    );
    assert_eq!(timeless(&tab.run(&["history", "--last", "1"]).success().stdout()), format!("{}\n", lines[3]));
    assert_eq!(
        timeless(&tab.run(&["history", "--failed", "--last", "1"]).success().stdout()),
        format!("{}\n", lines[1])
    );
}

fn history_show_prints_one_command_with_its_output(tab: &ShellTab, numbers: &str, seq: &str) {
    assert_eq!(timeless(&tab.run(&["history", "show", "5"]).success().stdout()), format!("{numbers}{seq}\n"));
    let missing = tab.run(&["history", "show", "9"]);
    assert_eq!(missing.code(), 1);
    assert_eq!(missing.stderr(), "error: no command 9 in work t1; list them with `agent-terminal history`\n");
}

fn a_command_the_users_history_leaves_out_is_recorded_as_typed(shell: &'static str) {
    let tab = ShellTab::new(shell);
    leave_spaced_and_secret_commands_out_of_the_users_history(&tab, shell);
    tab.open().success();
    let cwd = tab.cwd();
    for line in ["echo first", " echo spaced", "echo secret word", "echo first"] {
        tab.run(&["exec", line]).success();
    }
    assert_eq!(
        timeless(&tab.run(&["history"]).success().stdout()),
        format!(
            "1 · exit 0 · <time> · {cwd} · echo first\n\
             2 · exit 0 · <time> · {cwd} ·  echo spaced\n\
             3 · exit 0 · <time> · {cwd} · echo secret word\n\
             4 · exit 0 · <time> · {cwd} · echo first\n"
        )
    );
    if shell == "bash" {
        bash_still_leaves_them_out_of_its_own_history(&tab);
    }
}

fn leave_spaced_and_secret_commands_out_of_the_users_history(tab: &ShellTab, shell: &str) {
    let (file, leave_out) = match shell {
        "zsh" => (".zshrc", "setopt HIST_IGNORE_SPACE\n"),
        "bash" => (".bashrc", "HISTCONTROL=ignoreboth\nHISTIGNORE='echo secret*'\n"),
        _ => (".config/fish/config.fish", ""),
    };
    let file = tab.home.path().join(file);
    std::fs::create_dir_all(file.parent().unwrap()).expect("create the config folder");
    std::fs::write(&file, leave_out).expect("write a startup file");
}

fn bash_still_leaves_them_out_of_its_own_history(tab: &ShellTab) {
    let settings = tab.run(&["exec", "echo \"[$HISTCONTROL] [$HISTIGNORE]\""]).success().stdout();
    assert_eq!(settings.lines().next(), Some("[ignoreboth] [echo secret*]"));
    let kept = tab.run(&["exec", "history"]).success().stdout();
    assert!(kept.contains("echo first") && !kept.contains("spaced") && !kept.contains("echo secret"), "{kept}");
}

#[test]
fn exec_and_history_print_json() {
    let tab = ShellTab::opened("bash");
    let cwd = tab.cwd();
    exec_prints_the_command_as_json(&tab, &cwd);
    let checked = tab.run(&["--json", "exec", "--check", "false"]);
    assert_eq!(checked.code(), 1);
    assert_eq!(checked.json()["check"], true);
    assert_eq!(checked.json()["exit_code"], 1);

    let history = tab.run(&["--json", "history", "--failed"]).success().json();
    assert_eq!(history["session"], "work");
    assert_eq!(history["tab"], "t1");
    the_failed_command_is_listed_without_its_output(&history);
}

fn exec_prints_the_command_as_json(tab: &ShellTab, cwd: &str) {
    let mut ran = tab.run(&["--json", "exec", "echo hi"]).success().json();
    let started = ran["started_at_ms"].as_u64().expect("started_at_ms");
    let ended = ran["ended_at_ms"].as_u64().expect("ended_at_ms");
    assert!(started <= ended, "{ran}");
    let first_frame = ran["first_frame"].as_u64().expect("first_frame");
    assert!(ran["last_frame"].as_u64().expect("last_frame") >= first_frame, "{ran}");
    ran["duration_ms"].as_u64().expect("duration_ms");
    let masked = serde_json::json!("<masked>");
    for timing in ["started_at_ms", "ended_at_ms", "duration_ms", "first_frame", "last_frame"] {
        ran[timing] = masked.clone();
    }
    assert_eq!(
        ran,
        serde_json::json!({
            "session": "work", "tab": "t1", "id": 1, "command": "echo hi", "cwd": cwd,
            "started_at_ms": masked, "ended_at_ms": masked, "duration_ms": masked, "exit_code": 0,
            "first_frame": masked, "last_frame": masked, "output": "hi\n"
        })
    );
}

fn the_failed_command_is_listed_without_its_output(history: &serde_json::Value) {
    let commands = history["commands"].as_array().expect("commands");
    assert_eq!(commands.len(), 1, "{history}");
    assert_eq!(commands[0]["id"], 2);
    assert_eq!(commands[0]["command"], "false");
    assert_eq!(commands[0]["exit_code"], 1);
    assert!(commands[0].get("output").is_none(), "history lists commands without their output: {history}");
}
