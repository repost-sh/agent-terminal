use super::tab::ShellTab;

in_every_shell!(the_command_log_keeps_every_finished_command_with_its_output);

fn logged_commands(tab: &ShellTab) -> Vec<serde_json::Value> {
    let path = tab.at.home().join("sessions/default/t1.commands.jsonl");
    let log = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    log.lines().map(|line| serde_json::from_str(line).expect("a command log line")).collect()
}

fn without_timing(mut logged: serde_json::Value) -> serde_json::Value {
    let started = logged["started_at_ms"].as_u64().expect("started_at_ms");
    assert!(logged["ended_at_ms"].as_u64().expect("ended_at_ms") >= started, "{logged}");
    logged["duration_ms"].as_u64().expect("duration_ms");
    let first_frame = logged["first_frame"].as_u64().expect("first_frame");
    assert!(logged["last_frame"].as_u64().expect("last_frame") >= first_frame, "{logged}");
    for timing in ["started_at_ms", "ended_at_ms", "duration_ms", "first_frame", "last_frame"] {
        logged[timing] = serde_json::json!("<masked>");
    }
    logged
}

fn finished(id: u32, command: &str, cwd: &str, exit_code: i32, output: &str) -> serde_json::Value {
    let masked = "<masked>";
    serde_json::json!({
        "id": id, "command": command, "cwd": cwd, "started_at_ms": masked, "ended_at_ms": masked,
        "duration_ms": masked, "exit_code": exit_code, "first_frame": masked, "last_frame": masked, "output": output
    })
}

fn the_command_log_keeps_every_finished_command_with_its_output(shell: &'static str) {
    let tab = ShellTab::opened(shell);
    let cwd = tab.cwd();
    tab.run(&["exec", "echo passing"]).success();
    tab.run(&["exec", "echo failing; false"]).success();
    let logged = logged_commands(&tab);
    assert_eq!(
        logged.iter().cloned().map(without_timing).collect::<Vec<_>>(),
        [finished(1, "echo passing", &cwd, 0, "passing\n"), finished(2, "echo failing; false", &cwd, 1, "failing\n")]
    );
    for line in &logged {
        history_show_prints_the_logged_command(&tab, line);
    }
    a_running_command_is_logged_once_it_finishes(&tab, &cwd);
    tab.run(&["restart"]).success();
    tab.run(&["exec", "echo again"]).success();
    let logged = logged_commands(&tab);
    assert_eq!(logged.len(), 4, "restart keeps the log of the tab");
    assert_eq!(without_timing(logged[3].clone()), finished(4, "echo again", &cwd, 0, "again\n"));
}

fn history_show_prints_the_logged_command(tab: &ShellTab, line: &serde_json::Value) {
    let mut shown = tab.run(&["--json", "history", "show", &line["id"].to_string()]).success().json();
    let shown = shown.as_object_mut().expect("a command");
    shown.remove("session");
    shown.remove("tab");
    assert_eq!(line, &serde_json::Value::Object(shown.clone()));
}

fn a_running_command_is_logged_once_it_finishes(tab: &ShellTab, cwd: &str) {
    let hold = tab.hold();
    assert_eq!(tab.run(&["exec", "cat hold", "--timeout", "300"]).code(), 3);
    assert_eq!(logged_commands(tab).len(), 2, "a running command is not in the log yet");
    std::fs::write(&hold, "released\n").expect("release the command");
    tab.run(&["wait", "--command"]).success();
    assert_eq!(without_timing(logged_commands(tab)[2].clone()), finished(3, "cat hold", cwd, 0, "released\n"));
}
