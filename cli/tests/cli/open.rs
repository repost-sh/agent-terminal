use crate::support::{AgentTerminal, fixture};

#[test]
fn open_prints_the_programs_first_stable_screen() {
    let at = AgentTerminal::new();
    let run = at
        .run(&["open", "--size", "40x10", "--wait", "line 3", &fixture("line-printer"), "--lines", "3", "--hold"])
        .success();
    insta::assert_snapshot!(run.screen(), @r"
    default t1 · line-printer · 40x10 · cursor 4,1 · frame N
     1│line 1
     2│line 2
     3│line 3
     …│ (rows 4-10 blank)
    ");
}

#[test]
fn snapshot_reads_a_full_screen_app() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui")]).success();
    let run = at.run(&["snapshot"]).success();
    insta::assert_snapshot!(run.screen(), @r"
    default t1 · fixture-tui · 50x8 · alt-screen · cursor hidden · frame N
    1│┌Files─────────────┐┌Details─────────────────────┐
    2││alpha.txt         ││Selected: alpha.txt         │
    3││beta.txt          ││                            │
    4││gamma.txt         ││                            │
    5││delta.txt         ││                            │
    6││                  ││                            │
    7│└──────────────────┘└────────────────────────────┘
    8│Move: j/k | Quit: q | Crash: x
    ");
}

#[test]
fn close_stops_the_program_and_leaves_nothing_to_read() {
    let at = AgentTerminal::new();
    at.run(&["open", &fixture("line-printer"), "--hold"]).success();
    let closed = at.run(&["close"]).success();
    assert_eq!(closed.stdout(), "closed default t1\n");
    let after = at.run(&["snapshot"]);
    assert_eq!(after.code(), 1);
    assert!(after.stderr().contains("no terminal is open in session default"), "{}", after.stderr());
}

#[test]
fn close_all_closes_every_session() {
    let at = AgentTerminal::new();
    at.run(&["--session", "a", "open", &fixture("line-printer"), "--hold"]).success();
    at.run(&["--session", "b", "open", &fixture("line-printer"), "--hold"]).success();
    let closed = at.run(&["close", "--all"]).success();
    assert_eq!(closed.stdout(), "closed a t1\nclosed b t1\n");
    assert_eq!(at.run(&["--session", "b", "snapshot"]).code(), 1);
}

#[test]
fn json_output_for_open_snapshot_and_close() {
    let at = AgentTerminal::new();
    let opened = at
        .run(&[
            "--json",
            "open",
            "--size",
            "30x5",
            "--wait",
            "line 2",
            &fixture("line-printer"),
            "--lines",
            "2",
            "--hold",
        ])
        .success()
        .json();
    assert_eq!(opened["session"], "default");
    assert_eq!(opened["tab"], "t1");
    assert_eq!(opened["program"], "line-printer");
    assert_eq!((opened["cols"].as_u64(), opened["rows"].as_u64()), (Some(30), Some(5)));
    assert_eq!(opened["alt_screen"], false);
    assert_eq!(opened["cursor"], serde_json::json!([3, 1]));
    assert_eq!(opened["lines"], serde_json::json!(["line 1", "line 2", "", "", ""]));

    let snapshot = at.run(&["--json", "snapshot"]).success().json();
    assert_eq!(snapshot["lines"], opened["lines"]);

    let closed = at.run(&["--json", "close"]).success().json();
    assert_eq!(closed, serde_json::json!({ "closed": [{ "session": "default", "tab": "t1" }] }));
}

#[test]
fn cwd_and_env_reach_the_program() {
    let at = AgentTerminal::new();
    let run = at
        .run(&[
            "open",
            "--size",
            "60x4",
            "--cwd",
            &at.home().to_string_lossy(),
            "--env",
            "GREETING=hello from env",
            "--wait",
            "hello from env",
            "sh",
            "-c",
            "basename \"$(pwd -P)\"; echo \"$GREETING\"; read _",
        ])
        .success();
    let folder = at.home().canonicalize().unwrap().file_name().unwrap().to_string_lossy().into_owned();
    let stdout = run.stdout();
    let lines: Vec<&str> = stdout.lines().skip(1).take(2).map(|l| l.split_once('│').unwrap().1).collect();
    assert_eq!(lines, vec![folder.as_str(), "hello from env"]);
}

#[test]
fn a_program_that_exits_shows_its_exit_code() {
    let at = AgentTerminal::new();
    let run = at
        .run(&[
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
    insta::assert_snapshot!(run.screen(), @r"
    default t1 · line-printer · 20x3 · cursor 2,1 · frame N · exited 7
    1│line 1
    …│ (rows 2-3 blank)
    ");
}

#[test]
fn opening_a_missing_program_fails_with_command_not_found() {
    let at = AgentTerminal::new();
    let run = at.run(&["open", "no-such-program-xyz"]);
    assert_eq!(run.code(), 1);
    assert_eq!(run.stderr(), "error: command not found: no-such-program-xyz\n");
}

#[test]
fn snapshot_with_nothing_open_explains_how_to_start() {
    let at = AgentTerminal::new();
    let run = at.run(&["snapshot"]);
    assert_eq!(run.code(), 1);
    assert_eq!(
        run.stderr(),
        "error: no terminal is open in session default; start one with `agent-terminal open <program>`\n"
    );
}

#[test]
fn a_program_that_prints_nothing_opens_with_a_blank_screen() {
    let at = AgentTerminal::new();
    let started = std::time::Instant::now();
    let run = at.run(&["open", "--size", "20x2", "cat"]).success();
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "took {:?}", started.elapsed());
    insta::assert_snapshot!(run.screen(), @r"
    default t1 · cat · 20x2 · cursor 1,1 · frame N
    …│ (rows 1-2 blank)
    ");
}

#[test]
fn the_daemon_keeps_no_file_its_first_caller_inherited() {
    let at = AgentTerminal::new();
    let home = at.home().to_path_buf();
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let output = std::process::Command::new("sh")
            .args(["-c", "exec 3>&1; exec \"$0\" open --size 20x2 cat", env!("CARGO_BIN_EXE_agent-terminal")])
            .env("AGENT_TERMINAL_HOME", home)
            .env_remove("AGENT_TERMINAL_SESSION")
            .output()
            .expect("run agent-terminal through sh");
        done.send(output.status.code()).expect("report the exit code");
    });
    let code = finished
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the caller is still waiting for the end of its output: the daemon holds a copy of the caller's pipe");
    assert_eq!(code, Some(0));
}

#[test]
fn waiting_for_text_that_never_appears_times_out_and_shows_the_screen() {
    let at = AgentTerminal::new();
    let run = at.run(&[
        "--timeout",
        "500",
        "open",
        "--size",
        "20x2",
        "--wait",
        "never shown",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--hold",
    ]);
    assert_eq!(run.code(), 3);
    assert_eq!(run.stdout(), "");
    let stderr = run.stderr();
    assert!(
        stderr.starts_with(
            "error: timed out after 500 ms waiting for \"never shown\" to appear; default t1 is still running"
        ),
        "{stderr}"
    );
    assert!(stderr.contains("\ndefault t1 · line-printer · 20x2 · "), "{stderr}");
    at.run(&["wait", "line 1"]).success();
}

#[test]
fn open_waits_for_a_stable_first_screen_by_default() {
    let at = AgentTerminal::new();
    let run = at.run(&["--timeout", "200", "open", "--size", "20x2", "cat"]);
    assert_eq!(run.code(), 3, "{}", run.stderr());
    assert!(
        run.stderr().starts_with(
            "error: timed out after 200 ms waiting for the screen to stay unchanged for 300 ms; default t1 is still running"
        ),
        "{}",
        run.stderr()
    );
}

#[test]
fn open_no_wait_prints_the_screen_without_waiting() {
    let at = AgentTerminal::new();
    let run = at.run(&["--timeout", "200", "open", "--no-wait", "--size", "30x4", &fixture("ticker"), "--flash"]);
    assert_eq!(run.code(), 0, "{}", run.stderr());
    assert!(run.stdout().starts_with("default t1 · ticker · 30x4 · "), "{}", run.stdout());
    at.run(&["wait", "tick 3"]).success();
}

#[test]
fn open_wait_stable_takes_an_optional_quiet_period() {
    let at = AgentTerminal::new();
    let run = at.run(&["--timeout", "200", "open", "--wait-stable", "--size", "20x2", "cat"]);
    assert_eq!(run.code(), 3, "{}", run.stderr());
    assert!(run.stderr().contains("waiting for the screen to stay unchanged for 300 ms;"), "{}", run.stderr());
    let run = at.run(&["open", "--wait-stable", "50", "--size", "20x2", "cat"]).success();
    assert!(run.stdout().starts_with("default t2 · cat · 20x2 · "), "{}", run.stdout());
}

#[test]
fn open_no_wait_goes_alone() {
    let at = AgentTerminal::new();
    let run = at.run(&["open", "--no-wait", "--wait", "x", "cat"]);
    assert_eq!(run.code(), 1, "{}", run.stderr());
    assert!(run.stderr().contains("cannot be used with"), "{}", run.stderr());
}

#[test]
fn open_wait_fails_with_exit_code_5_when_the_program_exits_without_the_text() {
    let at = AgentTerminal::new();
    let run = at.run(&[
        "open",
        "--size",
        "20x3",
        "--wait",
        "never shown",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--exit",
        "7",
    ]);
    assert_eq!(run.code(), 5, "{}", run.stderr());
    let stderr = run.stderr();
    assert!(
        stderr.starts_with(
            "error: stopped waiting for \"never shown\" to appear: line-printer in default t1 has exited with code 7\n"
        ),
        "{stderr}"
    );
    assert!(stderr.contains("1│line 1\n"), "{stderr}");
}

#[test]
fn simultaneous_first_calls_share_one_daemon() {
    let at = AgentTerminal::new();
    std::thread::scope(|scope| {
        let opens: Vec<_> = (0..6).map(|_| scope.spawn(|| at.run(&["open", "--size", "20x2", "cat"]).code())).collect();
        let codes: Vec<i32> = opens.into_iter().map(|open| open.join().unwrap()).collect();
        assert_eq!(codes, vec![0; 6]);
    });
    let closed = at.run(&["close", "--all"]).success().stdout();
    assert_eq!(closed.lines().count(), 6, "every open must land in the same daemon:\n{closed}");
}
