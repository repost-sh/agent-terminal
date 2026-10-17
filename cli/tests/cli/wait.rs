use crate::support::{AgentTerminal, fixture, open_tui};

fn numbers_as_n(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_number = false;
    for c in text.chars() {
        if c.is_ascii_digit() {
            if !in_number {
                out.push('N');
            }
            in_number = true;
        } else {
            out.push(c);
            in_number = false;
        }
    }
    out
}

fn open_line_printer(at: &AgentTerminal, extra: &[&str]) {
    let printer = fixture("line-printer");
    let mut args = vec!["open", "--name", "app", "--size", "30x5", "--wait", "line 1", printer.as_str()];
    args.extend_from_slice(extra);
    at.run(&args).success();
}

#[test]
fn wait_returns_once_the_text_is_on_screen() {
    let at = AgentTerminal::new();
    open_line_printer(&at, &["--lines", "3", "--delay-ms", "200", "--hold"]);
    let waited = at.run(&["wait", "line 3"]).success();
    assert_eq!(numbers_as_n(&waited.stdout()), "waited N ms for \"line N\" to appear in work app\n");
    assert_eq!(at.run(&["snapshot"]).success().stdout().lines().nth(3), Some("3│line 3"));
}

#[test]
fn wait_sees_text_that_is_on_screen_for_a_single_frame() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "30x4", "--wait", "ready", &fixture("ticker"), "--flash"]).success();
    let waited = at.run(&["--json", "wait", "tick 60"]).success().json();
    assert_eq!((&waited["session"], &waited["tab"]), (&serde_json::json!("work"), &serde_json::json!("app")));
    assert_eq!(waited["frame"], 120, "tick n is the only thing on screen in frame 2n: {waited}");
}

#[test]
fn a_wait_that_times_out_exits_3_and_prints_the_last_screen() {
    let at = AgentTerminal::new();
    open_line_printer(&at, &["--lines", "2", "--hold"]);
    at.run(&["wait", "line 2"]).success();
    let waited = at.run(&["--timeout", "300", "wait", "never shown"]);
    assert_eq!(waited.code(), 3, "{}", waited.stderr());
    assert_eq!(waited.stdout(), "");
    let stderr = waited.stderr();
    assert!(
        stderr.starts_with(
            "error: timed out after 300 ms waiting for \"never shown\" to appear; work app is still running"
        ),
        "{stderr}"
    );
    assert!(stderr.contains("1│line 1\n2│line 2\n"), "{stderr}");
}

#[test]
fn wait_assert_exits_2_when_the_condition_is_not_met_in_time() {
    let at = AgentTerminal::new();
    open_line_printer(&at, &["--lines", "2", "--hold"]);
    at.run(&["wait", "line 2"]).success();

    let asserted = at.run(&["--timeout", "300", "wait", "never shown", "--assert"]);
    assert_eq!(asserted.code(), 2, "{}", asserted.stderr());
    assert_eq!(asserted.stdout(), "");
    let stderr = asserted.stderr();
    assert!(stderr.starts_with("error: \"never shown\" did not appear within 300 ms in work app\n"), "{stderr}");
    assert!(stderr.contains("1│line 1\n2│line 2\n"), "{stderr}");
    wait_gone_assert_fails_in_json_and_a_met_condition_passes(&at);
}

fn wait_gone_assert_fails_in_json_and_a_met_condition_passes(at: &AgentTerminal) {
    let stable = at.run(&["--timeout", "300", "--json", "wait", "--gone", "line 1", "--assert"]);
    assert_eq!(stable.code(), 2, "{}", stable.stdout());
    assert_eq!(stable.json()["error"]["code"], 2);
    assert_eq!(stable.json()["error"]["message"], "\"line 1\" did not disappear within 300 ms in work app");

    let met = at.run(&["wait", "line 1", "--assert"]).success();
    assert_eq!(numbers_as_n(&met.stdout()), "waited N ms for \"line N\" to appear in work app\n");
}

#[test]
fn wait_regex_matches_a_pattern() {
    let at = AgentTerminal::new();
    open_line_printer(&at, &["--lines", "3", "--delay-ms", "100", "--hold"]);
    let waited = at.run(&["wait", r"^line [3-9]$", "--regex"]).success();
    assert_eq!(numbers_as_n(&waited.stdout()), "waited N ms for a match for /^line [N-N]$/ in work app\n");
    assert_eq!(at.run(&["snapshot"]).success().stdout().lines().nth(3), Some("3│line 3"));
}

#[test]
fn wait_regex_refuses_an_invalid_pattern_with_exit_code_1() {
    let at = AgentTerminal::new();
    open_line_printer(&at, &["--lines", "1", "--hold"]);
    let waited = at.run(&["wait", "line (", "--regex"]);
    assert_eq!(waited.code(), 1, "{}", waited.stderr());
    assert!(waited.stderr().starts_with("error: invalid regex \"line (\": "), "{}", waited.stderr());
}

#[test]
fn wait_gone_returns_once_the_text_has_left_the_screen() {
    let at = AgentTerminal::new();
    open_tui(&at);
    at.run(&["press", "j"]).success();
    let waited = at.run(&["wait", "--gone", "Selected: alpha.txt"]).success();
    assert_eq!(numbers_as_n(&waited.stdout()), "waited N ms for \"Selected: alpha.txt\" to disappear in work app\n");
    at.run(&["wait", "Selected: beta.txt"]).success();
}

#[test]
fn wait_gone_times_out_while_the_text_stays() {
    let at = AgentTerminal::new();
    open_line_printer(&at, &["--lines", "1", "--hold"]);
    let waited = at.run(&["--timeout", "200", "wait", "line 1", "--gone"]);
    assert_eq!(waited.code(), 3, "{}", waited.stderr());
    assert!(
        waited.stderr().starts_with("error: timed out after 200 ms waiting for \"line 1\" to disappear;"),
        "{}",
        waited.stderr()
    );
}

#[test]
fn wait_rows_only_looks_at_those_rows() {
    let at = AgentTerminal::new();
    open_line_printer(&at, &["--lines", "3", "--hold"]);
    at.run(&["wait", "line 3"]).success();
    let outside = at.run(&["--timeout", "200", "wait", "line 1", "--rows", "2:3"]);
    assert_eq!(outside.code(), 3, "{}", outside.stderr());
    assert!(
        outside.stderr().starts_with("error: timed out after 200 ms waiting for \"line 1\" to appear in rows 2-3;"),
        "{}",
        outside.stderr()
    );
    let inside = at.run(&["wait", "line 2", "--rows", "2:3"]).success();
    assert_eq!(numbers_as_n(&inside.stdout()), "waited N ms for \"line N\" to appear in rows N-N in work app\n");
    at.run(&["wait", "--gone", "line 1", "--rows", "2:3"]).success();
}

#[test]
fn wait_rows_refuses_a_range_that_is_not_first_to_last() {
    let at = AgentTerminal::new();
    for (rows, problem) in [
        ("3:2", "rows count from 1, and A must not be after B"),
        ("0:2", "rows count from 1, and A must not be after B"),
        ("2", "expected A:B"),
    ] {
        let waited = at.run(&["wait", "x", "--rows", rows]);
        assert_eq!(waited.code(), 1, "{}", waited.stderr());
        assert!(waited.stderr().contains(problem), "{rows}: {}", waited.stderr());
    }
}

fn open_ticker(at: &AgentTerminal, mode: &str) {
    at.run(&["open", "--name", "app", "--size", "30x4", "--wait", "ready", &fixture("ticker"), mode, "--every", "20"])
        .success();
}

#[test]
fn wait_stable_returns_once_the_screen_stops_changing() {
    let at = AgentTerminal::new();
    open_line_printer(&at, &["--lines", "3", "--hold"]);
    at.run(&["wait", "line 3"]).success();
    let waited = at.run(&["wait", "--stable"]).success();
    assert_eq!(numbers_as_n(&waited.stdout()), "waited N ms for the screen to stay unchanged for N ms in work app\n");
    assert!(waited.stdout().contains(" for 300 ms in "), "{}", waited.stdout());
}

#[test]
fn wait_stable_ignores_output_that_redraws_the_same_screen() {
    let at = AgentTerminal::new();
    open_ticker(&at, "--repaint");
    at.run(&["wait", "--stable", "200"]).success();
}

#[test]
fn wait_stable_times_out_while_the_screen_keeps_changing() {
    let at = AgentTerminal::new();
    open_ticker(&at, "--flash");
    let waited = at.run(&["--timeout", "1500", "wait", "--stable", "1000"]);
    assert_eq!(waited.code(), 3, "{}", waited.stderr());
    assert!(
        waited.stderr().starts_with(
            "error: timed out after 1500 ms waiting for the screen to stay unchanged for 1000 ms; work app is still running"
        ),
        "{}",
        waited.stderr()
    );
}

#[test]
fn wait_idle_returns_once_the_program_prints_nothing() {
    let at = AgentTerminal::new();
    open_line_printer(&at, &["--lines", "3", "--hold"]);
    at.run(&["wait", "line 3"]).success();
    let waited = at.run(&["wait", "--idle"]).success();
    assert_eq!(numbers_as_n(&waited.stdout()), "waited N ms for the program to print nothing for N ms in work app\n");
    assert!(waited.stdout().contains(" for 300 ms in "), "{}", waited.stdout());
}

#[test]
fn wait_idle_times_out_while_the_program_keeps_printing_the_same_screen() {
    let at = AgentTerminal::new();
    open_ticker(&at, "--repaint");
    let waited = at.run(&["--timeout", "1500", "wait", "--idle", "1000"]);
    assert_eq!(waited.code(), 3, "{}", waited.stderr());
    assert!(
        waited.stderr().starts_with(
            "error: timed out after 1500 ms waiting for the program to print nothing for 1000 ms; work app is still running"
        ),
        "{}",
        waited.stderr()
    );
}

#[test]
fn wait_exit_returns_once_the_program_exits_and_shows_its_code() {
    let at = AgentTerminal::new();
    open_tui(&at);
    at.run(&["press", "x"]).success();
    let waited = at.run(&["wait", "--exit"]).success();
    assert_eq!(numbers_as_n(&waited.stdout()), "waited N ms for the program to exit in work app (exit code N)\n");
    let json = at.run(&["--json", "wait", "--exit", "--code", "3"]).success().json();
    assert_eq!(json["exit_code"], 3, "{json}");
}

#[test]
fn wait_exit_with_a_code_fails_with_exit_code_2_when_the_program_exits_with_another() {
    let at = AgentTerminal::new();
    open_tui(&at);
    at.run(&["press", "x"]).success();
    let waited = at.run(&["wait", "--exit", "--code", "0"]);
    assert_eq!(waited.code(), 2, "{}", waited.stderr());
    let stderr = waited.stderr();
    assert!(stderr.starts_with("error: fixture-tui in work app exited with code 3, not 0\n"), "{stderr}");
    assert!(stderr.contains("exited 3\n"), "{stderr}");
}

#[test]
fn wait_exit_times_out_while_the_program_runs() {
    let at = AgentTerminal::new();
    open_line_printer(&at, &["--lines", "1", "--hold"]);
    let waited = at.run(&["--timeout", "200", "wait", "--exit", "--code", "0"]);
    assert_eq!(waited.code(), 3, "{}", waited.stderr());
    assert!(
        waited.stderr().starts_with("error: timed out after 200 ms waiting for the program to exit with code 0;"),
        "{}",
        waited.stderr()
    );
}

#[test]
fn wait_code_only_goes_with_exit() {
    let at = AgentTerminal::new();
    let waited = at.run(&["wait", "--code", "0"]);
    assert_eq!(waited.code(), 1, "{}", waited.stderr());
    assert!(waited.stderr().contains("--exit"), "{}", waited.stderr());
}

#[test]
fn wait_cursor_returns_once_the_cursor_is_there() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "40x4", "--wait", "ready", &fixture("input-echo")]).success();
    at.run(&["type", "ab"]).success();
    let waited = at.run(&["wait", "--cursor", "2,3"]).success();
    assert_eq!(numbers_as_n(&waited.stdout()), "waited N ms for the cursor to reach N,N in work app\n");
    let elsewhere = at.run(&["--timeout", "200", "wait", "--cursor", "4,1"]);
    assert_eq!(elsewhere.code(), 3, "{}", elsewhere.stderr());
    assert!(elsewhere.stderr().contains("cursor 2,3 · frame"), "{}", elsewhere.stderr());
}

#[test]
fn wait_frame_returns_once_the_screen_has_changed_that_often() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "40x4", "--wait", "ready", &fixture("input-echo")]).success();
    at.run(&["wait", "--cursor", "2,1"]).success();
    let frame = at.run(&["--json", "snapshot"]).success().json()["frame"].as_u64().unwrap();
    let next = (frame + 1).to_string();
    let unchanged = at.run(&["--timeout", "200", "wait", "--frame", &next]);
    assert_eq!(unchanged.code(), 3, "{}", unchanged.stderr());
    assert!(
        unchanged.stderr().starts_with(&format!("error: timed out after 200 ms waiting for frame {next};")),
        "{}",
        unchanged.stderr()
    );
    at.run(&["type", "a"]).success();
    let waited = at.run(&["--json", "wait", "--frame", &next]).success().json();
    assert!(waited["frame"].as_u64().is_some_and(|reached| reached > frame), "{waited}");
}

#[test]
fn a_wait_fails_with_exit_code_5_once_the_program_has_exited_without_it() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "20x3", &fixture("line-printer"), "--lines", "1", "--exit", "7"])
        .success();
    let started = std::time::Instant::now();
    let waited = at.run(&["wait", "never shown"]);
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "took {:?}", started.elapsed());
    assert_eq!(waited.code(), 5, "{}", waited.stderr());
    let stderr = waited.stderr();
    assert!(
        stderr.starts_with(
            "error: stopped waiting for \"never shown\" to appear: line-printer in work app has exited with code 7\n"
        ),
        "{stderr}"
    );
    assert!(stderr.contains("1│line 1\n"), "{stderr}");
}

#[test]
fn wait_with_a_number_pauses_that_long_whatever_the_timeout() {
    let at = AgentTerminal::new();
    open_line_printer(&at, &["--lines", "1", "--hold"]);
    let started = std::time::Instant::now();
    let waited = at.run(&["--timeout", "100", "wait", "300"]).success();
    assert!(started.elapsed() >= std::time::Duration::from_millis(300), "took {:?}", started.elapsed());
    assert_eq!(numbers_as_n(&waited.stdout()), "waited N ms for N ms to pass in work app\n");
}

#[test]
fn input_wait_gone_returns_once_the_text_has_left_the_screen() {
    let at = AgentTerminal::new();
    open_tui(&at);
    let pressed = at.run(&["press", "j", "--wait-gone", "Selected: alpha.txt"]).success();
    assert_eq!(pressed.stdout(), "pressed 1 key in work app\n");
    at.run(&["wait", "Selected: beta.txt"]).success();
}

#[test]
fn input_wait_stable_returns_once_the_screen_has_settled_after_the_input() {
    let at = AgentTerminal::new();
    open_tui(&at);
    at.run(&["wait", "--stable", "100"]).success();
    for (key, selected) in [("j", "beta.txt"), ("j", "gamma.txt"), ("k", "beta.txt")] {
        let pressed = at.run(&["--json", "press", key, "--wait-stable", "100", "--snapshot"]).success().json();
        assert_eq!(pressed["snapshot"]["lines"][1], format!("│alpha.txt         ││Selected: {selected:<18}│"));
        assert_eq!(pressed["snapshot"]["frame"], at.run(&["--json", "get", "frame"]).success().json()["frame"]);
    }
}

#[test]
fn input_wait_stable_times_out_while_the_screen_keeps_changing() {
    let at = AgentTerminal::new();
    open_ticker(&at, "--flash");
    let sent = at.run(&["--timeout", "1500", "send", "x", "--wait-stable", "1000"]);
    assert_eq!(sent.code(), 3, "{}", sent.stderr());
    assert!(
        sent.stderr()
            .starts_with("error: timed out after 1500 ms waiting for the screen to stay unchanged for 1000 ms;"),
        "{}",
        sent.stderr()
    );
}

#[test]
fn input_snapshot_prints_the_screen_after_the_wait() {
    let at = AgentTerminal::new();
    open_tui(&at);
    let pressed = at.run(&["press", "j", "--wait", "Selected: beta.txt", "--snapshot"]).success();
    insta::assert_snapshot!(pressed.screen(), @r"
    pressed 1 key in work app
    work app · fixture-tui · 50x8 · alt-screen · cursor hidden · frame N
    1│┌Files─────────────┐┌Details─────────────────────┐
    2││alpha.txt         ││Selected: beta.txt          │
    3││beta.txt          ││Size: 50x8                  │
    4││gamma.txt         ││                            │
    5││delta.txt         ││                            │
    6││                  ││                            │
    7│└──────────────────┘└────────────────────────────┘
    8│Move: j/k | Quit: q | Crash: x
    ");
}

#[test]
fn input_snapshot_puts_the_screen_in_the_json_result() {
    let at = AgentTerminal::new();
    open_tui(&at);
    let pressed = at.run(&["--json", "press", "k", "j", "--wait", "Selected: beta.txt", "--snapshot"]).success().json();
    assert_eq!((&pressed["input"], &pressed["keys"]), (&serde_json::json!("keys"), &serde_json::json!(2)));
    assert_eq!(pressed["snapshot"]["lines"][1], "│alpha.txt         ││Selected: beta.txt          │");
}

#[test]
fn input_takes_one_wait_at_a_time() {
    let at = AgentTerminal::new();
    let pressed = at.run(&["press", "j", "--wait", "x", "--wait-stable"]);
    assert_eq!(pressed.code(), 1, "{}", pressed.stderr());
    assert!(pressed.stderr().contains("cannot be used with"), "{}", pressed.stderr());
}

#[test]
fn wait_and_expect_look_at_another_tab_with_tab() {
    let at = AgentTerminal::new();
    open_line_printer(&at, &["--lines", "2", "--hold"]);
    at.run(&["open", "--name", "cat", "--size", "20x2", "cat"]).success();
    let waited = at.run(&["wait", "line 2", "--tab", "app"]).success();
    assert!(waited.stdout().ends_with(" in work app\n"), "{}", waited.stdout());
    assert_eq!(
        at.run(&["expect", "line 2", "--tab", "app"]).success().stdout(),
        "\"line 2\" is on screen in work app\n"
    );
    assert_eq!(at.run(&["expect", "line 2"]).code(), 2);
}
