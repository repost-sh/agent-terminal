use crate::support::{AgentTerminal, fixture};

fn open_three_lines(at: &AgentTerminal) {
    at.run(&[
        "open",
        "--name",
        "app",
        "--size",
        "30x5",
        "--wait",
        "line 3",
        &fixture("line-printer"),
        "--lines",
        "3",
        "--hold",
    ])
    .success();
}

fn open_styled(at: &AgentTerminal) {
    let script = r"printf '\033[1;31mred\033[0m \033[7minv\033[0m \033[4;44mund\033[0m\n'; read _";
    at.run(&["open", "--name", "app", "--size", "30x3", "--wait", "und", "sh", "-c", script]).success();
}

fn without_frame(text: &str) -> String {
    match text.find("frame ") {
        Some(i) => {
            let end = text[i + 6..].find(|c: char| !c.is_ascii_digit()).map_or(text.len(), |e| i + 6 + e);
            format!("{}frame N{}", &text[..i], &text[end..])
        }
        None => text.to_string(),
    }
}

#[test]
fn expect_passes_when_the_text_is_on_screen() {
    let at = AgentTerminal::new();
    open_three_lines(&at);
    assert_eq!(at.run(&["expect", "line 2"]).success().stdout(), "\"line 2\" is on screen in work app\n");
    let json = at.run(&["--json", "expect", "line 2"]).success().json();
    assert_eq!(json, serde_json::json!({ "session": "work", "tab": "app", "fact": "\"line 2\" is on screen" }));
}

#[test]
fn expect_fails_with_exit_code_2_and_the_screen_when_the_text_is_missing() {
    let at = AgentTerminal::new();
    open_three_lines(&at);
    let checked = at.run(&["expect", "line 9"]);
    assert_eq!(checked.code(), 2, "{}", checked.stderr());
    assert_eq!(checked.stdout(), "");
    insta::assert_snapshot!(without_frame(&checked.stderr()), @r#"
    error: "line 9" is not on screen in work app
    work app · line-printer · 30x5 · cursor 4,1 · frame N
    1│line 1
    2│line 2
    3│line 3
    …│ (rows 4-5 blank)
    "#);
}

#[test]
fn expect_gone_passes_when_the_text_is_not_on_screen() {
    let at = AgentTerminal::new();
    open_three_lines(&at);
    let checked = at.run(&["expect", "--gone", "line 9"]).success();
    assert_eq!(checked.stdout(), "\"line 9\" is not on screen in work app\n");
}

#[test]
fn expect_gone_fails_with_exit_code_2_and_the_rows_that_show_the_text() {
    let at = AgentTerminal::new();
    open_three_lines(&at);
    let checked = at.run(&["expect", "line 2", "--gone"]);
    assert_eq!(checked.code(), 2, "{}", checked.stderr());
    insta::assert_snapshot!(without_frame(&checked.stderr()), @r#"
    error: "line 2" is on screen in work app, in row 2
    work app · line-printer · 30x5 · cursor 4,1 · frame N
    2│line 2
    "#);
    let several = at.run(&["expect", "--gone", "line"]);
    assert_eq!(several.code(), 2, "{}", several.stderr());
    assert!(
        several.stderr().starts_with("error: \"line\" is on screen in work app, in rows 1-3\n"),
        "{}",
        several.stderr()
    );
}

#[test]
fn expect_exit_code_passes_when_the_program_exited_with_it() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "20x3", &fixture("line-printer"), "--lines", "1", "--exit", "7"])
        .success();
    at.run(&["wait", "--exit"]).success();
    let checked = at.run(&["expect", "--exit-code", "7"]).success();
    assert_eq!(checked.stdout(), "line-printer exited with code 7 in work app\n");
}

#[test]
fn expect_exit_code_fails_with_exit_code_2_when_the_program_exited_with_another_code() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "20x3", &fixture("line-printer"), "--lines", "1", "--exit", "7"])
        .success();
    at.run(&["wait", "--exit"]).success();
    let checked = at.run(&["expect", "--exit-code", "0"]);
    assert_eq!(checked.code(), 2, "{}", checked.stderr());
    insta::assert_snapshot!(without_frame(&checked.stderr()), @r"
    error: line-printer in work app exited with code 7, not 0
    work app · line-printer · 20x3 · cursor 2,1 · frame N · exited 7
    1│line 1
    …│ (rows 2-3 blank)
    ");
}

#[test]
fn expect_exit_code_fails_with_exit_code_2_while_the_program_runs() {
    let at = AgentTerminal::new();
    open_three_lines(&at);
    let checked = at.run(&["expect", "--exit-code", "0"]);
    assert_eq!(checked.code(), 2, "{}", checked.stderr());
    assert!(
        checked
            .stderr()
            .starts_with("error: line-printer in work app is still running, so it has not exited with code 0\n"),
        "{}",
        checked.stderr()
    );
}

#[test]
fn expect_cell_style_passes_when_the_cell_has_that_style() {
    let at = AgentTerminal::new();
    open_styled(&at);
    let checked = at.run(&["expect", "--cell", "1,1", "--style", "bold,fg=red"]).success();
    assert_eq!(checked.stdout(), "cell 1,1 is bold, fg=red in work app\n");
    at.run(&["expect", "--cell", "1,5", "--style", "inverse"]).success();
    at.run(&["expect", "--cell", "1,9", "--style", "underline", "--style", "bg=blue"]).success();
}

#[test]
fn expect_cell_style_fails_with_exit_code_2_and_the_row_of_the_cell() {
    let at = AgentTerminal::new();
    open_styled(&at);
    let checked = at.run(&["expect", "--cell", "1,5", "--style", "bold,inverse"]);
    assert_eq!(checked.code(), 2, "{}", checked.stderr());
    insta::assert_snapshot!(without_frame(&checked.stderr()), @r#"
    error: cell 1,5 in work app is not bold: it is "i" fg=default bg=default inverse
    work app · sh · 30x3 · cursor 2,1 · frame N
    1│red inv und
    "#);
}

#[test]
fn expect_cell_outside_the_screen_is_an_error() {
    let at = AgentTerminal::new();
    open_styled(&at);
    let checked = at.run(&["expect", "--cell", "9,1", "--style", "bold"]);
    assert_eq!(checked.code(), 1, "{}", checked.stderr());
    assert_eq!(checked.stderr(), "error: cell 9,1 is outside the 30x3 screen\n");
}

#[test]
fn expect_style_refuses_an_unknown_attribute() {
    let at = AgentTerminal::new();
    let checked = at.run(&["expect", "--cell", "1,1", "--style", "blink"]);
    assert_eq!(checked.code(), 1, "{}", checked.stderr());
    assert!(
        checked.stderr().contains("unknown style blink; use bold, inverse, underline, fg=<color> or bg=<color>"),
        "{}",
        checked.stderr()
    );
}

#[test]
fn expect_needs_one_thing_to_check() {
    let at = AgentTerminal::new();
    for args in [&["expect"][..], &["expect", "x", "--exit-code", "0"], &["expect", "--cell", "1,1"]] {
        let checked = at.run(args);
        assert_eq!(checked.code(), 1, "{args:?}: {}", checked.stderr());
    }
}
