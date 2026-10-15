use crate::support::{AgentTerminal, fixture};

const COUNT_RUNS: &str = "echo $$ >> pids; echo run >> runs; n=$(wc -l < runs | tr -d ' '); \
                          echo \"run $n $GREETING in $(basename \"$(pwd -P)\") size $(stty size)\"; \
                          [ \"$n\" = 1 ] && [ -n \"$EXIT_FIRST\" ] && exit 3; read _";

fn workdir(at: &AgentTerminal) -> String {
    let dir = at.home().join("work");
    std::fs::create_dir_all(&dir).unwrap();
    dir.to_string_lossy().into_owned()
}

#[test]
fn restart_runs_the_same_program_again_in_the_same_cwd_env_and_size() {
    let at = AgentTerminal::new();
    let work = workdir(&at);
    at.run(&[
        "open",
        "--size",
        "40x4",
        "--cwd",
        &work,
        "--env",
        "GREETING=hello",
        "--wait",
        "run 1",
        "sh",
        "-c",
        COUNT_RUNS,
    ])
    .success();
    at.run(&["resize", "36x3", "--wait", "run 1"]).success();

    let restarted = at.run(&["restart", "--wait", "run 2"]).success();
    insta::assert_snapshot!(restarted.screen(), @r"
    default t1 · sh · 36x3 · cursor 2,1 · frame N
    1│run 2 hello in work size 3 36
    …│ (rows 2-3 blank)
    ");
    assert_eq!(at.run(&["tab"]).success().stdout(), "default t1 · sh · 36x3 · running · current\n");
}

#[test]
fn frame_numbers_go_on_after_a_restart() {
    let at = AgentTerminal::new();
    let work = workdir(&at);
    at.run(&[
        "open",
        "--size",
        "40x3",
        "--cwd",
        &work,
        "--env",
        "GREETING=hi",
        "--wait",
        "run 1",
        "sh",
        "-c",
        COUNT_RUNS,
    ])
    .success();
    let before: u64 = at.run(&["get", "frame"]).success().stdout().trim().parse().unwrap();

    at.run(&["restart", "--wait", "run 2"]).success();

    let after: u64 = at.run(&["get", "frame"]).success().stdout().trim().parse().unwrap();
    assert!(after > before, "frame numbers go on after a restart: {after} after {before}");
}

#[test]
fn restart_brings_back_a_program_that_exited() {
    let at = AgentTerminal::new();
    let work = workdir(&at);
    at.run(&[
        "open",
        "--size",
        "40x3",
        "--cwd",
        &work,
        "--env",
        "GREETING=hi",
        "--env",
        "EXIT_FIRST=1",
        "--no-wait",
        "sh",
        "-c",
        COUNT_RUNS,
    ])
    .success();
    at.run(&["wait", "--exit"]).success();
    let opened = at.run(&["snapshot"]).success();
    assert!(opened.stdout().starts_with("default t1 · sh · 40x3 · cursor 2,1 · frame "), "{}", opened.stdout());
    assert!(opened.stdout().lines().next().unwrap().ends_with(" · exited 3"), "{}", opened.stdout());

    let restarted = at.run(&["restart", "--wait", "run 2"]).success();
    insta::assert_snapshot!(restarted.screen(), @r"
    default t1 · sh · 40x3 · cursor 2,1 · frame N
    1│run 2 hi in work size 3 40
    …│ (rows 2-3 blank)
    ");
}

#[test]
fn restart_reaches_a_tab_other_than_the_current_one() {
    let at = AgentTerminal::new();
    let work = workdir(&at);
    at.run(&[
        "open",
        "--size",
        "40x3",
        "--cwd",
        &work,
        "--env",
        "GREETING=hi",
        "--wait",
        "run 1",
        "sh",
        "-c",
        COUNT_RUNS,
    ])
    .success();
    at.run(&["tab", "new", "--size", "20x2", "--wait", "line 1", &fixture("line-printer"), "--lines", "1", "--hold"])
        .success();

    let restarted = at.run(&["--tab", "t1", "restart", "--wait", "run 2"]).success();
    assert!(restarted.stdout().starts_with("default t1 · sh · 40x3 · "), "{}", restarted.stdout());
    assert_eq!(at.run(&["--json", "tab"]).success().json()["current"], "t2");
}

#[test]
fn restarts_of_one_tab_at_the_same_time_leave_exactly_one_program_running() {
    let at = AgentTerminal::new();
    let work = workdir(&at);
    at.run(&[
        "open",
        "--size",
        "40x3",
        "--cwd",
        &work,
        "--env",
        "GREETING=hi",
        "--wait",
        "run 1",
        "sh",
        "-c",
        COUNT_RUNS,
    ])
    .success();

    std::thread::scope(|scope| {
        let restarts: Vec<_> = (0..3).map(|_| scope.spawn(|| at.run(&["restart", "--wait", "run"]).code())).collect();
        let codes: Vec<i32> = restarts.into_iter().map(|restart| restart.join().unwrap()).collect();
        assert_eq!(codes, vec![0; 3]);
    });

    assert_eq!(at.run(&["tab"]).success().stdout(), "default t1 · sh · 40x3 · running · current\n");
    let pids = std::fs::read_to_string(at.home().join("work/pids")).unwrap();
    let alive: Vec<&str> = pids
        .lines()
        .filter(|pid| std::process::Command::new("kill").args(["-0", pid]).status().unwrap().success())
        .collect();
    assert_eq!(alive.len(), 1, "programs still running: {alive:?} of {pids}");
}
