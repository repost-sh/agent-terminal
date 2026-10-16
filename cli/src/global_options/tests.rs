use std::time::Duration;

use super::{parse_duration, without_global_options};

#[test]
fn an_idle_timeout_is_raw_milliseconds_or_seconds_minutes_or_hours() {
    let parsed = ["0", "250", "250ms", "30s", "10m", "1h"].map(|value| parse_duration(value).unwrap());
    assert_eq!(parsed, [0, 250, 250, 30_000, 600_000, 3_600_000].map(Duration::from_millis));
    let refusal = |value: &str| parse_duration(value).unwrap_err();
    assert_eq!(refusal("5d"), "invalid time: 5d (expected a duration like 30s, 10m, 1h or raw ms (0 = never))");
    assert_eq!(
        refusal("h"),
        "invalid time: h (expected a duration like 30s, 10m, 1h or raw ms (0 = never)): cannot parse integer from empty string"
    );
    assert_eq!(
        refusal("18446744073709551615h"),
        "invalid time: 18446744073709551615h (expected a duration like 30s, 10m, 1h or raw ms (0 = never))"
    );
}

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
        without_global_options(&typed(&["--idle-timeout", "1h", "--screenshot-dir=/s", "--unknown"])),
        typed(&["--unknown"])
    );
}
