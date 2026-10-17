use super::without_global_options;

#[test]
fn a_call_line_drops_the_global_options_and_their_values_but_keeps_everything_after_a_separator() {
    let typed = |words: &[&str]| words.iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(
        without_global_options(&typed(&[
            "--session",
            "work",
            "--json",
            "open",
            "--timeout=5",
            "--tab",
            "2",
            "vim",
            "--size",
            "80x24"
        ])),
        typed(&["open", "vim", "--size", "80x24"])
    );
    assert_eq!(
        without_global_options(&typed(&["press", "--", "--session", "x", "--json"])),
        typed(&["press", "--", "--session", "x", "--json"])
    );
    assert_eq!(
        without_global_options(&typed(&["--timeout", "5", "--screenshot-dir=/s", "--unknown"])),
        typed(&["--unknown"])
    );
}
