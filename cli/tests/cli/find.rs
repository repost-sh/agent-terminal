use crate::support::{AgentTerminal, fixture};

fn open_eight_lines_in_five_rows(at: &AgentTerminal) {
    at.run(&["open", "--size", "30x5", "--wait", "line 8", &fixture("line-printer"), "--lines", "8", "--hold"])
        .success();
}

#[test]
fn find_prints_row_and_column_of_every_match_on_screen() {
    let at = AgentTerminal::new();
    open_eight_lines_in_five_rows(&at);
    assert_eq!(at.run(&["find", "line 6"]).success().stdout(), "2,1 line 6\n");
    assert_eq!(at.run(&["find", "ine"]).success().stdout(), "1,2 ine\n2,2 ine\n3,2 ine\n4,2 ine\n");
    assert_eq!(
        at.run(&["--json", "find", "line 6"]).success().json(),
        serde_json::json!({ "matches": [{ "row": 2, "col": 1, "text": "line 6" }] })
    );
}

#[test]
fn find_regex_matches_a_pattern() {
    let at = AgentTerminal::new();
    open_eight_lines_in_five_rows(&at);
    assert_eq!(at.run(&["find", "--regex", "line [57]"]).success().stdout(), "1,1 line 5\n3,1 line 7\n");
    assert_eq!(at.run(&["find", "line [57]"]).code(), 1);
    let invalid = at.run(&["find", "--regex", "line ("]);
    assert_eq!(invalid.code(), 1);
    assert!(invalid.stderr().starts_with("error: invalid regex \"line (\":"), "{}", invalid.stderr());
}

#[test]
fn find_all_includes_scrollback_above_the_screen() {
    let at = AgentTerminal::new();
    open_eight_lines_in_five_rows(&at);
    find_without_all_looks_only_at_the_screen(&at);
    assert_eq!(at.run(&["find", "--all", "line 2"]).success().stdout(), "-3,1 line 2\n");
    assert_eq!(at.run(&["find", "--all", "--regex", "line [48]"]).success().stdout(), "-1,1 line 4\n4,1 line 8\n");
    let nowhere = at.run(&["find", "--all", "line 9"]);
    assert_eq!(nowhere.code(), 1);
    assert_eq!(nowhere.stderr(), "error: \"line 9\" is not on the screen or in its scrollback\n");
}

fn find_without_all_looks_only_at_the_screen(at: &AgentTerminal) {
    let missing = at.run(&["find", "line 2"]);
    assert_eq!(missing.code(), 1);
    assert_eq!(missing.stderr(), "error: \"line 2\" is not on the screen\n");
}

#[test]
fn find_counts_columns_in_cells_and_finds_repeats_on_one_row() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "30x3", "--wait", "ready", "sh", "-c", "printf '日本 ab ab ready'; read _"]).success();
    assert_eq!(at.run(&["find", "ab"]).success().stdout(), "1,6 ab\n1,9 ab\n");
}
