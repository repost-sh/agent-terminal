use serde_json::{Value, json};

use crate::native::daemon::tests::{ECHO, Running, on, open, text};
use crate::native::dashboard::Backend;
use crate::native::store::TabId;

fn wait(wait: &Value, timeout_ms: u64, assert: bool) -> Value {
    on("wait", &json!({ "wait": wait, "timeout_ms": timeout_ms, "assert": assert }))
}

fn waited(running: &Running, request: &Value) -> Value {
    let mut outcome = running.ok(request);
    assert!(outcome["waited_ms"].take().is_u64(), "{outcome}");
    outcome["frame"].take();
    outcome
}

#[test]
fn a_wait_names_what_it_waited_for_and_an_exit_wait_its_exit_code() {
    let running = Running::new();
    running.open("echo", ECHO);
    assert_eq!(
        waited(&running, &wait(&text("ready"), 10_000, false)),
        json!({ "session": "s", "tab": "echo", "until": "\"ready\" to appear", "waited_ms": null, "frame": null })
    );
    running.ok(&open("exits", "printf 'ready\\n'; exit 7"));
    assert_eq!(
        waited(&running, &wait(&json!({ "until": "exit", "code": 7 }), 10_000, false)),
        json!({ "session": "s", "tab": "exits", "until": "the program to exit with code 7", "waited_ms": null, "frame": null, "exit_code": 7 })
    );
    let other_code = running.refused(&wait(&json!({ "until": "exit", "code": 8 }), 10_000, false));
    assert_eq!(other_code, (2, "sh in s exits exited with code 7, not 8".to_string()));
    let elapsed = waited(&running, &wait(&json!({ "until": "elapsed", "ms": 5 }), 0, false));
    assert_eq!(elapsed["until"], "5 ms to pass");
}

fn timed_out(running: &Running, waiting: &Value) -> String {
    let (code, message) = running.refused(&wait(waiting, 20, false));
    let (asserted, assertion) = running.refused(&wait(waiting, 20, true));
    let waited = message.strip_prefix("timed out after 20 ms waiting for ").unwrap_or(&message);
    let still =
        "; s echo is still running (read it with `agent-terminal snapshot`, stop it with `agent-terminal close`)";
    format!("{code} {} | {asserted} {assertion}", waited.strip_suffix(still).unwrap_or(waited))
}

#[test]
fn a_wait_that_times_out_says_what_it_waited_for_and_an_asserted_one_what_did_not_happen() {
    let running = Running::new();
    running.open("echo", ECHO);
    let rows = json!({ "first": 1, "last": 2 });
    let waits = [
        json!({ "until": "stable", "quiet_ms": 60_000 }),
        json!({ "until": "idle", "quiet_ms": 60_000 }),
        text("never"),
        json!({ "until": "text", "text": "nev+er", "regex": true, "rows": rows }),
        json!({ "until": "gone", "text": "ready", "regex": false, "rows": null }),
        json!({ "until": "gone", "text": "rea.y", "regex": true, "rows": rows }),
        json!({ "until": "exit", "code": null }),
        json!({ "until": "exit", "code": 3 }),
        json!({ "until": "cursor", "row": 9, "col": 9 }),
        json!({ "until": "frame", "frame": 999_999 }),
    ];
    insta::assert_snapshot!(waits.map(|waiting| timed_out(&running, &waiting)).join("\n"), @r#"
        3 the screen to stay unchanged for 60000 ms | 2 the screen did not stay unchanged for 60000 ms within 20 ms in s echo
        3 the program to print nothing for 60000 ms | 2 the program did not stop printing for 60000 ms within 20 ms in s echo
        3 "never" to appear | 2 "never" did not appear within 20 ms in s echo
        3 a match for /nev+er/ in rows 1-2 | 2 nothing matched /nev+er/ in rows 1-2 within 20 ms in s echo
        3 "ready" to disappear | 2 "ready" did not disappear within 20 ms in s echo
        3 no match for /rea.y/ in rows 1-2 | 2 still a match for /rea.y/ in rows 1-2 within 20 ms in s echo
        3 the program to exit | 2 the program did not exit within 20 ms in s echo
        3 the program to exit with code 3 | 2 the program did not exit with code 3 within 20 ms in s echo
        3 the cursor to reach 9,9 | 2 the cursor did not reach 9,9 within 20 ms in s echo
        3 frame 999999 | 2 the screen did not reach frame 999999 within 20 ms in s echo
    "#);
}

#[test]
fn a_wait_on_a_program_that_exits_stops_with_how_it_exited() {
    let running = Running::new();
    running.ok(&open("exits", "printf 'ready\\n'; exit 4"));
    let (code, message) = running.refused(&wait(&text("never"), 10_000, false));
    assert_eq!(
        (code, message.as_str()),
        (5, "stopped waiting for \"never\" to appear: sh in s exits has exited with code 4")
    );
}

#[test]
fn a_command_wait_needs_a_shell_with_its_integration() {
    let running = Running::new();
    running.open("echo", ECHO);
    let refusal = "s echo runs sh without shell integration, so it keeps no command history; exec, history and wait --command work in a tab opened with `agent-terminal shell`";
    let exec = on("exec", &json!({ "line": "ls", "check": false, "timeout_ms": 100 }));
    let history = on("history", &json!({ "last": null, "failed": false }));
    assert_eq!(
        [
            running.refused(&wait(&json!({ "until": "command" }), 100, false)),
            running.refused(&exec),
            running.refused(&history)
        ],
        [(1, refusal.to_string()), (1, refusal.to_string()), (1, refusal.to_string())]
    );
}

#[test]
fn waiting_for_control_returns_at_once_without_a_human_and_times_out_while_one_holds_it() {
    let running = Running::new();
    running.open("echo", ECHO);
    let wait_control =
        |timeout_ms: u64, assert: bool| on("wait-control", &json!({ "timeout_ms": timeout_ms, "assert": assert }));
    let back = waited(&running, &wait_control(10_000, false));
    assert_eq!(
        back,
        json!({ "session": "s", "tab": "echo", "until": "a human to hand back control", "waited_ms": null, "frame": null })
    );
    let hold = running.daemon.take_control(&"s".parse().unwrap(), "echo").unwrap();
    assert_eq!(
        [running.refused(&wait_control(20, false)), running.refused(&wait_control(20, true))],
        [
            (3, "timed out after 20 ms waiting for a human to hand back control of s echo; wait longer with --timeout <ms>".to_string()),
            (2, "a human did not hand back control of s echo within 20 ms".to_string())
        ]
    );
    let typed = on(
        "input",
        &json!({ "input": { "kind": "type", "text": "x", "delay_ms": 0 }, "wait": null, "timeout_ms": 100 }),
    );
    assert_eq!(
        running.refused(&typed),
        (4, "A human has control of s echo. Run `agent-terminal wait --control`.".to_string())
    );
    let sessions = running.daemon.sessions.lock().unwrap();
    sessions["s"].tabs[&TabId::FIRST].control.close();
    drop((sessions, hold));
    assert_eq!(
        running.refused(&wait_control(10_000, false)),
        (1, "stopped waiting for control: s echo was closed".to_string())
    );
}

fn exec(line: &str, check: bool, timeout_ms: u64) -> Value {
    on("exec", &json!({ "line": line, "check": check, "timeout_ms": timeout_ms }))
}

fn ran(running: &Running, request: &Value) -> String {
    let ran = running.ok(request);
    format!(
        "{} {} command {} {:?} exit {} check {} output {:?}",
        ran["session"].as_str().unwrap(),
        ran["tab"].as_str().unwrap(),
        ran["id"],
        ran["command"].as_str().unwrap(),
        ran["exit_code"],
        ran.get("check").is_some(),
        ran["output"].as_str().unwrap()
    )
}

#[test]
fn exec_runs_a_line_at_the_prompt_and_answers_with_its_record_and_output() {
    let running = Running::new();
    running.ok(&running.shell("shell", 10_000));
    assert_eq!(
        [ran(&running, &exec("printf 'hi\\n'", false, 10_000)), ran(&running, &exec("false", true, 10_000))],
        [
            "s shell command 1 \"printf 'hi\\\\n'\" exit 0 check false output \"hi\\n\"",
            "s shell command 2 \"false\" exit 1 check true output \"\""
        ]
    );
}

#[test]
fn exec_times_out_on_a_line_still_running_and_then_on_the_busy_shell() {
    let running = Running::new();
    running.ok(&running.shell("shell", 10_000));
    let (code, still) = running.refused(&exec("sleep 30", false, 1_000));
    let (busy_code, busy) = running.refused(&exec("true", false, 100));
    insta::assert_snapshot!(format!("{code} {still}\n{busy_code} {busy}"), @r"
        3 timed out after 1000 ms waiting for `sleep 30` to finish in s shell; it is still running as command 1 (wait for it with `agent-terminal wait --command`, read its output with `agent-terminal history show 1`, interrupt it with `agent-terminal signal INT`)
        3 timed out after 100 ms waiting for `sleep 30` to finish in s shell, so `true` did not run (read the screen with `agent-terminal snapshot`, interrupt a command with `agent-terminal signal INT`)
    ");
}

#[test]
fn exec_stops_when_the_shell_exits_and_a_shell_without_a_prompt_names_its_startup_files() {
    let running = Running::new();
    running.ok(&running.shell("shell", 10_000));
    assert_eq!(
        running.refused(&exec("exit 4", false, 10_000)),
        (5, "stopped waiting for the shell command to finish: bash in s shell has exited with code 4".to_string())
    );
    std::fs::write(running.user().join(".bashrc"), "read waiting\n").unwrap();
    let (code, message) = running.refused(&running.shell("again", 300));
    assert_eq!(code, 3);
    insta::assert_snapshot!(message, @"timed out after 300 ms waiting for the first prompt of bash in s again; its startup files may be waiting for input or the shell integration did not load (read the screen with `agent-terminal snapshot`)");
}
