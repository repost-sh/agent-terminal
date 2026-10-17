use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use crate::support::{AgentTerminal, fixture, open_tui};

const NAME_THE_SESSION: &str = "error: name this conversation's session after its task and create it first: `agent-terminal session new repost-forward`, then pass `--session repost-forward` on every call (or export AGENT_TERMINAL_SESSION)\n";

#[test]
fn open_without_a_session_fails_with_how_to_name_and_create_one_and_starts_nothing() {
    let at = AgentTerminal::without_session();
    let refused = at.run(&["open", "--name", "app", "--size", "20x2", "cat"]);
    assert_eq!((refused.code(), refused.stdout(), refused.stderr()), (1, String::new(), NAME_THE_SESSION.to_string()));
    let named_badly = at.run(&["--session", "Repost_Forward", "open", "--name", "app", "cat"]);
    assert_eq!(named_badly.code(), 1);
    assert!(
        named_badly.stderr().contains(
            "\"Repost_Forward\" is not a name: a name is 1 to 24 characters of a-z, 0-9 and -, starting with a letter or digit"
        ),
        "{}",
        named_badly.stderr()
    );
    assert!(!at.home().join("daemon.sock").exists(), "a refused open must not start the daemon");
}

#[test]
fn open_in_a_session_that_was_not_created_fails_and_creates_nothing() {
    let at = AgentTerminal::new();
    let refused = at.run(&["--session", "typo", "open", "--name", "app", "--size", "20x2", "cat"]);
    assert_eq!(
        (refused.code(), refused.stderr()),
        (1, "error: no session typo; create it with `agent-terminal session new typo`\n".to_string())
    );
    assert_eq!(at.run(&["session", "list"]).success().stdout(), "work · no tabs\n");
}

#[test]
fn open_prints_the_programs_first_stable_screen() {
    let at = AgentTerminal::new();
    let script = "printf 'line 1\\nline 2\\nline 3'; read _";
    let run = at.run(&["open", "--name", "app", "--size", "40x10", "--wait", "line 3", "sh", "-c", script]).success();
    insta::assert_snapshot!(run.screen(), @r"
    work app · sh · 40x10 · cursor 3,7 · frame N
     1│line 1
     2│line 2
     3│line 3
     …│ (rows 4-10 blank)
    ");
}

#[test]
fn options_of_open_after_the_program_are_open_options_and_the_rest_goes_to_the_program() {
    let at = AgentTerminal::new();
    let printer = fixture("line-printer");
    at.run(&["open", "--name", "app", &printer, "--lines", "2", "--size", "30x4", "--hold", "--wait", "line 2"])
        .success();
    at.run(&["wait", "--cursor", "3,1"]).success();
    insta::assert_snapshot!(at.run(&["snapshot"]).success().screen(), @r"
    work app · line-printer · 30x4 · cursor 3,1 · frame N
    1│line 1
    2│line 2
    …│ (rows 3-4 blank)
    ");

    let script = "printf \"got $1 $2\"; read _";
    let passed = at
        .run(&[
            "tab", "new", "--name", "api", "sh", "-c", script, "sh", "--size", "30x3", "--wait", "got", "--", "--size",
            "--wait",
        ])
        .success();
    insta::assert_snapshot!(passed.screen(), @r"
    work api · sh · 30x3 · cursor 1,18 · frame N
    1│got --size --wait
    …│ (rows 2-3 blank)
    ");

    let global = at.run(&["open", &printer, "--hold", "--json", "--wait=line 3", "--name", "web"]).success().json();
    assert_eq!((&global["tab"], &global["lines"][2]), (&serde_json::json!("web"), &serde_json::json!("line 3")));
}

#[test]
fn snapshot_reads_a_full_screen_app() {
    let at = AgentTerminal::new();
    open_tui(&at);
    let run = at.run(&["snapshot"]).success();
    insta::assert_snapshot!(run.screen(), @r"
    work app · fixture-tui · 50x8 · alt-screen · cursor hidden · frame N
    1│┌Files─────────────┐┌Details─────────────────────┐
    2││alpha.txt         ││Selected: alpha.txt         │
    3││beta.txt          ││Size: 50x8                  │
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
    at.run(&["open", "--name", "app", &fixture("line-printer"), "--hold"]).success();
    let closed = at.run(&["close"]).success();
    assert_eq!(closed.stdout(), "closed work app\n");
    let after = at.run(&["snapshot"]);
    assert_eq!(after.code(), 1);
    assert!(after.stderr().contains("no terminal is open in session work"), "{}", after.stderr());
}

const IGNORES_HUP: &str = "trap '' HUP; echo ready; exec sleep 600";

fn running(pid: &str) -> bool {
    Command::new("kill").args(["-0", pid]).stderr(Stdio::null()).status().expect("run kill").success()
}

#[test]
fn close_and_close_all_stop_a_program_that_ignores_sighup() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--wait", "ready", "sh", "-c", IGNORES_HUP]).success();
    let first = at.run(&["get", "pid"]).success().stdout().trim().to_string();
    at.run(&["session", "new", "other"]).success();
    at.run(&["--session", "other", "open", "--name", "app", "--wait", "ready", "sh", "-c", IGNORES_HUP]).success();
    let second = at.run(&["--session", "other", "get", "pid"]).success().stdout().trim().to_string();
    assert!(running(&first) && running(&second));

    at.run(&["close"]).success();
    assert!(!running(&first), "close left {first} running");
    at.run(&["--session", "other", "close", "--all"]).success();
    assert!(!running(&second), "close --all left {second} running");
}

#[test]
fn restart_stops_a_program_that_ignores_sighup_and_runs_it_again() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--wait", "ready", "sh", "-c", IGNORES_HUP]).success();
    let before = at.run(&["get", "pid"]).success().stdout().trim().to_string();
    at.run(&["--timeout", "5000", "restart", "--wait", "ready"]).success();
    assert!(!running(&before), "restart left {before} running");
    assert_ne!(at.run(&["get", "pid"]).success().stdout().trim(), before);
}

#[test]
fn json_output_for_open_snapshot_and_close() {
    let at = AgentTerminal::new();
    let script = "printf 'line 1\\nline 2'; read _";
    let opened = at
        .run(&["--json", "open", "--name", "app", "--size", "30x5", "--wait", "line 2", "sh", "-c", script])
        .success()
        .json();
    the_opened_tab_is_described(&opened);
    the_opened_screen_is_described(&opened);

    let snapshot = at.run(&["--json", "snapshot"]).success().json();
    assert_eq!(snapshot["lines"], opened["lines"]);

    let closed = at.run(&["--json", "close"]).success().json();
    assert_eq!(closed, serde_json::json!({ "closed": [{ "session": "work", "tab": "app" }] }));
}

fn the_opened_tab_is_described(opened: &serde_json::Value) {
    assert_eq!(opened["session"], "work");
    assert_eq!(opened["tab"], "app");
    assert_eq!(opened["program"], "sh");
    assert_eq!((opened["cols"].as_u64(), opened["rows"].as_u64()), (Some(30), Some(5)));
}

fn the_opened_screen_is_described(opened: &serde_json::Value) {
    assert_eq!(opened["alt_screen"], false);
    assert_eq!(opened["cursor"], serde_json::json!([2, 7]));
    assert_eq!(opened["lines"], serde_json::json!(["line 1", "line 2", "", "", ""]));
}

#[test]
fn cwd_and_env_reach_the_program() {
    let at = AgentTerminal::new();
    let run = at
        .run(&[
            "open",
            "--name",
            "app",
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
fn term_and_colorterm_describe_the_emulated_terminal_unless_env_sets_them() {
    let at = AgentTerminal::new();
    let script = ["sh", "-c", "echo \"[$TERM] [$COLORTERM]\"; read _"];
    let mut emulated = vec!["open", "--name", "app", "--size", "40x3", "--wait", "]"];
    emulated.extend(script);
    let from_caller = [("TERM", Path::new("dumb")), ("COLORTERM", Path::new("16"))];
    let opened = at.run_with_env(&emulated, &from_caller).success();
    assert_eq!(opened.stdout().lines().nth(1), Some("1│[xterm-256color] [truecolor]"));

    let mut chosen = vec![
        "tab",
        "new",
        "--name",
        "api",
        "--size",
        "40x3",
        "--env",
        "TERM=vt100",
        "--env",
        "COLORTERM=",
        "--wait",
        "]",
    ];
    chosen.extend(script);
    let opened = at.run_with_env(&chosen, &from_caller).success();
    assert_eq!(opened.stdout().lines().nth(1), Some("1│[vt100] []"));
}

#[test]
fn a_relative_cwd_is_relative_to_where_open_runs_and_a_missing_one_is_refused() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    std::fs::create_dir(work.join("app")).expect("create the app folder");
    let script = ["sh", "-c", "echo \"in $(pwd -P)\"; read _"];
    at.run_in(at.home(), &["open", "--name", "app", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    let mut relative = vec!["tab", "new", "--name", "api", "--size", "100x3", "--cwd", "app", "--wait", "in "];
    relative.extend(script);
    let opened = at.run_in(&work, &relative).success();
    assert_eq!(opened.stdout().lines().nth(1), Some(format!("1│in {}", work.join("app").display()).as_str()));

    for (cwd, shown) in
        [("missing", work.join("missing").display().to_string()), ("/nowhere/at/all", "/nowhere/at/all".into())]
    {
        let mut args = vec!["open", "--name", "web", "--cwd", cwd];
        args.extend(script);
        let refused = at.run_in(&work, &args);
        assert_eq!(refused.code(), 1, "{}", refused.stdout());
        assert_eq!(refused.stderr(), format!("error: the working directory {shown} does not exist\n"));
    }
    assert_eq!(at.run(&["tab"]).success().stdout().lines().count(), 2, "a refused open opens no tab");
}

#[test]
fn a_program_that_exits_shows_its_exit_code() {
    let at = AgentTerminal::new();
    let printer = fixture("line-printer");
    let run = at
        .run(&[
            "open",
            "--name",
            "app",
            "--size",
            "20x3",
            "--wait-stable",
            "5000",
            &printer,
            "--lines",
            "1",
            "--exit",
            "7",
        ])
        .success();
    insta::assert_snapshot!(run.screen(), @r"
    work app · line-printer · 20x3 · cursor 2,1 · frame N · exited 7
    1│line 1
    …│ (rows 2-3 blank)
    ");
}

#[test]
fn opening_a_missing_program_fails_with_command_not_found() {
    let at = AgentTerminal::new();
    let run = at.run(&["open", "--name", "app", "no-such-program-xyz"]);
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
        "error: no terminal is open in session work; start one with `agent-terminal open --name <name> <program>`\n"
    );
}

#[test]
fn open_waits_for_the_first_output_of_a_program_that_starts_slowly() {
    let at = AgentTerminal::new();
    let late = ["--start-after-ms", "600", "--lines", "1", "--hold"];
    let run =
        at.run(&[&["open", "--name", "app", "--size", "20x2", &fixture("line-printer")], &late[..]].concat()).success();
    insta::assert_snapshot!(run.screen(), @r"
    work app · line-printer · 20x2 · cursor 2,1 · frame N
    1│line 1
    …│ (row 2 blank)
    ");
}

#[test]
fn restart_waits_for_the_first_output_of_the_program_run_again() {
    let at = AgentTerminal::new();
    let late = ["--start-after-ms", "600", "--lines", "1", "--hold"];
    at.run(
        &[&["open", "--name", "app", "--size", "20x2", "--wait", "line 1", &fixture("line-printer")], &late[..]]
            .concat(),
    )
    .success();
    let run = at.run(&["restart"]).success();
    insta::assert_snapshot!(run.screen(), @r"
    work app · line-printer · 20x2 · cursor 2,1 · frame N
    1│line 1
    …│ (row 2 blank)
    ");
}

#[test]
fn a_program_that_prints_nothing_opens_with_a_blank_screen_that_says_so() {
    let at = AgentTerminal::new();
    let started = std::time::Instant::now();
    let run = at.run(&["open", "--name", "app", "--size", "20x2", "cat"]).success();
    assert!(started.elapsed() < Duration::from_secs(4), "took {:?}", started.elapsed());
    insta::assert_snapshot!(run.screen(), @r"
    work app · cat · 20x2 · cursor 1,1 · frame N · nothing printed yet
    …│ (rows 1-2 blank)
    ");
    let opened = at.run(&["--json", "open", "--name", "cat", "--size", "20x2", "cat"]).success().json();
    assert_eq!(opened["nothing_printed"], true);
}

#[test]
fn waiting_for_text_that_never_appears_times_out_and_shows_the_screen() {
    let at = AgentTerminal::new();
    let printer = fixture("line-printer");
    let run = at.run(&[
        "--timeout",
        "500",
        "open",
        "--name",
        "app",
        "--size",
        "20x2",
        "--wait",
        "never shown",
        &printer,
        "--lines",
        "1",
        "--hold",
    ]);
    assert_eq!(run.code(), 3);
    assert_eq!(run.stdout(), "");
    let stderr = run.stderr();
    assert!(
        stderr.starts_with(
            "error: timed out after 500 ms waiting for \"never shown\" to appear; work app is still running"
        ),
        "{stderr}"
    );
    assert!(stderr.contains("\nwork app · line-printer · 20x2 · "), "{stderr}");
    at.run(&["wait", "line 1"]).success();
}

#[test]
fn open_waits_for_a_stable_first_screen_by_default() {
    let at = AgentTerminal::new();
    let ticker = fixture("ticker");
    at.run(&["open", "--name", "warm", "--wait", "ready", &ticker]).success();
    let run = at.run(&["--timeout", "250", "open", "--name", "app", "--size", "20x2", &ticker, "--flash"]);
    assert_eq!(run.code(), 3, "{}", run.stderr());
    assert!(
        run.stderr().starts_with(
            "error: timed out after 250 ms waiting for the screen to stay unchanged for 300 ms; work app is still running"
        ),
        "{}",
        run.stderr()
    );
}

#[test]
fn open_no_wait_prints_the_screen_without_waiting() {
    let at = AgentTerminal::new();
    let ticker = fixture("ticker");
    let run = at.run(&["--timeout", "200", "open", "--name", "app", "--no-wait", "--size", "30x4", &ticker, "--flash"]);
    assert_eq!(run.code(), 0, "{}", run.stderr());
    assert!(run.stdout().starts_with("work app · ticker · 30x4 · "), "{}", run.stdout());
    at.run(&["wait", "tick 3"]).success();
}

#[test]
fn open_wait_stable_takes_an_optional_quiet_period() {
    let at = AgentTerminal::new();
    let ticker = fixture("ticker");
    at.run(&["open", "--name", "warm", "--wait", "ready", &ticker]).success();
    let run =
        at.run(&["--timeout", "250", "open", "--name", "app", "--wait-stable", "--size", "20x2", &ticker, "--flash"]);
    assert_eq!(run.code(), 3, "{}", run.stderr());
    assert!(run.stderr().contains("waiting for the screen to stay unchanged for 300 ms;"), "{}", run.stderr());
    let run = at.run(&["open", "--name", "api", "--wait-stable", "50", "--size", "20x2", "cat"]).success();
    assert!(run.stdout().starts_with("work api · cat · 20x2 · "), "{}", run.stdout());
}

#[test]
fn open_no_wait_goes_alone() {
    let at = AgentTerminal::new();
    let run = at.run(&["open", "--name", "app", "--no-wait", "--wait", "x", "cat"]);
    assert_eq!(run.code(), 1, "{}", run.stderr());
    assert!(run.stderr().contains("cannot be used with"), "{}", run.stderr());
}

#[test]
fn open_wait_prints_the_last_screen_when_the_program_exits_without_the_text() {
    let at = AgentTerminal::new();
    let printer = fixture("line-printer");
    let run = at
        .run(&[
            "open",
            "--name",
            "app",
            "--size",
            "20x3",
            "--wait",
            "never shown",
            &printer,
            "--lines",
            "1",
            "--exit",
            "7",
        ])
        .success();
    insta::assert_snapshot!(run.screen(), @r"
    work app · line-printer · 20x3 · cursor 2,1 · frame N · exited 7
    1│line 1
    …│ (rows 2-3 blank)
    ");
}

#[test]
fn simultaneous_first_calls_share_one_daemon() {
    let at = &AgentTerminal::unstarted();
    let names = ["s1", "s2", "s3", "s4", "s5", "s6"];
    std::thread::scope(|scope| {
        let news: Vec<_> = names.map(|name| scope.spawn(move || at.run(&["session", "new", name]).code())).into();
        let codes: Vec<i32> = news.into_iter().map(|new| new.join().unwrap()).collect();
        assert_eq!(codes, vec![0; 6]);
    });
    let listed = at.run(&["session", "list"]).success().stdout();
    assert_eq!(listed, names.map(|name| format!("{name} · no tabs\n")).concat(), "every call must land in one daemon");
}

#[test]
fn a_pipe_the_caller_hands_to_the_call_that_starts_the_daemon_reaches_end_of_file_when_the_call_exits() {
    let at = AgentTerminal::unstarted();
    let mut open = Command::new("sh")
        .args(["-c", r#"exec "$0" "$@" 3>&1"#, env!("CARGO_BIN_EXE_agent-terminal"), "session", "new", "work"])
        .env("AGENT_TERMINAL_HOME", at.home())
        .env_remove("AGENT_TERMINAL_SESSION")
        .stdout(Stdio::piped())
        .spawn()
        .expect("run agent-terminal session new with its stdout pipe also on descriptor 3");
    let mut pipe = open.stdout.take().expect("the stdout pipe");
    let (sender, end_of_file) = mpsc::channel();
    std::thread::spawn(move || {
        let mut printed = String::new();
        let read = pipe.read_to_string(&mut printed).map(|_| printed);
        sender.send(read).expect("hand back what open printed");
    });
    assert!(open.wait().expect("wait for session new").success());
    let printed = end_of_file
        .recv_timeout(Duration::from_secs(10))
        .expect("the pipe stayed open after session new exited, so the daemon holds a descriptor of it")
        .expect("read the pipe");
    assert_eq!(printed, "created session work\n");
}
