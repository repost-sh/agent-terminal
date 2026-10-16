use std::process::Command;

struct Checked {
    code: i32,
    stdout: String,
    stderr: String,
}

fn check(results: &str) -> std::io::Result<Checked> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("results.json");
    std::fs::write(&path, results)?;
    let output = Command::new(env!("CARGO_BIN_EXE_at-perf")).arg("check").arg(&path).output()?;
    Ok(Checked {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

#[test]
fn medians_within_the_targets_and_close_to_the_base_pass() {
    let checked = check(
        r#"{"metrics": [
            {"metric": "round-trip", "base": [3.0, 2.0, 9.0], "change": [3.1, 2.5, 3.2]},
            {"metric": "snapshot", "base": [3.5, 3.4, 3.6], "change": [3.5, 3.6, 3.4]},
            {"metric": "png", "base": [5.0, 5.2, 30.0], "change": [5.1, 5.0, 5.2]},
            {"metric": "press-wait-text", "base": [0.4, 0.8, 0.5, 0.7], "change": [0.6, 0.6, 0.6, 0.6]},
            {"metric": "idle-memory", "base": [5.4], "change": [5.4]},
            {"metric": "tab-memory", "base": [3.7], "change": [3.8]}
        ]}"#,
    )
    .expect("run at-perf check");

    assert_eq!(
        checked.stdout,
        "| metric | target | base | this change | verdict |\n\
         |---|---|---|---|---|\n\
         | CLI round trip (`get frame`) | ≤ 5 ms | 3.00 ms | 3.10 ms | ok |\n\
         | `snapshot` | ≤ 10 ms | 3.50 ms | 3.50 ms | ok |\n\
         | `screenshot` PNG at 120x40 | ≤ 40 ms | 5.20 ms | 5.10 ms | ok |\n\
         | `press --wait <text>` after the text appears | none | 0.60 ms | 0.60 ms | ok |\n\
         | idle daemon memory | ≤ 10 MB | 5.40 MB | 5.40 MB | ok |\n\
         | memory per active tab | about 5 MB (reference) | 3.70 MB | 3.80 MB | ok |\n"
    );
    assert_eq!(checked.code, 0, "stderr: {}", checked.stderr);
}

#[test]
fn a_median_over_its_target_fails_even_when_the_base_misses_it_too() {
    let checked = check(
        r#"{"metrics": [
            {"metric": "round-trip", "base": [3.0], "change": [3.0]},
            {"metric": "snapshot", "base": [12.0, 12.5, 11.5], "change": [11.0, 13.0, 12.0]},
            {"metric": "png", "base": [5.0], "change": [5.0]},
            {"metric": "press-wait-text", "base": [0.6], "change": [0.6]},
            {"metric": "idle-memory", "base": [5.4], "change": [5.4]},
            {"metric": "tab-memory", "base": [5.0], "change": [5.0]}
        ]}"#,
    )
    .expect("run at-perf check");

    assert_eq!(
        checked.stdout,
        "| metric | target | base | this change | verdict |\n\
         |---|---|---|---|---|\n\
         | CLI round trip (`get frame`) | ≤ 5 ms | 3.00 ms | 3.00 ms | ok |\n\
         | `snapshot` | ≤ 10 ms | 12.00 ms | 12.00 ms | over target |\n\
         | `screenshot` PNG at 120x40 | ≤ 40 ms | 5.00 ms | 5.00 ms | ok |\n\
         | `press --wait <text>` after the text appears | none | 0.60 ms | 0.60 ms | ok |\n\
         | idle daemon memory | ≤ 10 MB | 5.40 MB | 5.40 MB | ok |\n\
         | memory per active tab | about 5 MB (reference) | 5.00 MB | 5.00 MB | ok |\n\
         \n\
         - `snapshot` is 12.00 ms, over its 10 ms target\n"
    );
    assert_eq!(checked.code, 2, "stderr: {}", checked.stderr);
}

#[test]
fn a_median_more_than_the_tolerance_over_the_base_is_a_regression() {
    let checked = check(
        r#"{"metrics": [
            {"metric": "round-trip", "base": [2.0, 2.1, 1.9], "change": [2.6, 2.5, 2.7]},
            {"metric": "snapshot", "base": [2.0], "change": [2.4]},
            {"metric": "png", "base": [30.0], "change": [45.0]},
            {"metric": "press-wait-text", "base": [0.2], "change": [0.4]},
            {"metric": "idle-memory", "base": [4.85], "change": [5.05]},
            {"metric": "tab-memory", "base": [3.0], "change": [3.4]}
        ]}"#,
    )
    .expect("run at-perf check");

    assert_eq!(
        checked.stdout,
        "| metric | target | base | this change | verdict |\n\
         |---|---|---|---|---|\n\
         | CLI round trip (`get frame`) | ≤ 5 ms | 2.00 ms | 2.60 ms | regressed |\n\
         | `snapshot` | ≤ 10 ms | 2.00 ms | 2.40 ms | ok |\n\
         | `screenshot` PNG at 120x40 | ≤ 40 ms | 30.00 ms | 45.00 ms | over target, regressed |\n\
         | `press --wait <text>` after the text appears | none | 0.20 ms | 0.40 ms | ok |\n\
         | idle daemon memory | ≤ 10 MB | 4.85 MB | 5.05 MB | ok |\n\
         | memory per active tab | about 5 MB (reference) | 3.00 MB | 3.40 MB | regressed |\n\
         \n\
         - CLI round trip (`get frame`) regressed: 2.60 ms against 2.00 ms on the base (+30%; the limit is +25% and +0.25 ms)\n\
         - `screenshot` PNG at 120x40 is 45.00 ms, over its 40 ms target\n\
         - `screenshot` PNG at 120x40 regressed: 45.00 ms against 30.00 ms on the base (+50%; the limit is +25% and +0.25 ms)\n\
         - memory per active tab regressed: 3.40 MB against 3.00 MB on the base (+13%; the limit is +10% and +0.25 MB)\n"
    );
    assert_eq!(checked.code, 2, "stderr: {}", checked.stderr);
}

#[test]
fn memory_per_tab_over_its_reference_passes_when_it_has_not_regressed() {
    let checked = check(
        r#"{"metrics": [
            {"metric": "round-trip", "base": [2.2], "change": [2.2]},
            {"metric": "snapshot", "base": [2.1], "change": [2.1]},
            {"metric": "png", "base": [6.3], "change": [6.3]},
            {"metric": "press-wait-text", "base": [0.35], "change": [0.35]},
            {"metric": "idle-memory", "base": [4.9], "change": [4.9]},
            {"metric": "tab-memory", "base": [9.0, 8.9, 9.1], "change": [9.0, 9.2, 8.8]}
        ]}"#,
    )
    .expect("run at-perf check");

    assert_eq!(
        checked.stdout,
        "| metric | target | base | this change | verdict |\n\
         |---|---|---|---|---|\n\
         | CLI round trip (`get frame`) | ≤ 5 ms | 2.20 ms | 2.20 ms | ok |\n\
         | `snapshot` | ≤ 10 ms | 2.10 ms | 2.10 ms | ok |\n\
         | `screenshot` PNG at 120x40 | ≤ 40 ms | 6.30 ms | 6.30 ms | ok |\n\
         | `press --wait <text>` after the text appears | none | 0.35 ms | 0.35 ms | ok |\n\
         | idle daemon memory | ≤ 10 MB | 4.90 MB | 4.90 MB | ok |\n\
         | memory per active tab | about 5 MB (reference) | 9.00 MB | 9.00 MB | over reference |\n"
    );
    assert_eq!(checked.code, 0, "stderr: {}", checked.stderr);
}

#[test]
fn memory_per_tab_over_its_reference_fails_when_it_regressed() {
    let checked = check(
        r#"{"metrics": [
            {"metric": "round-trip", "base": [2.2], "change": [2.2]},
            {"metric": "snapshot", "base": [2.1], "change": [2.1]},
            {"metric": "png", "base": [6.3], "change": [6.3]},
            {"metric": "press-wait-text", "base": [0.35], "change": [0.35]},
            {"metric": "idle-memory", "base": [4.9], "change": [4.9]},
            {"metric": "tab-memory", "base": [9.0], "change": [10.8]}
        ]}"#,
    )
    .expect("run at-perf check");

    assert_eq!(
        checked.stdout,
        "| metric | target | base | this change | verdict |\n\
         |---|---|---|---|---|\n\
         | CLI round trip (`get frame`) | ≤ 5 ms | 2.20 ms | 2.20 ms | ok |\n\
         | `snapshot` | ≤ 10 ms | 2.10 ms | 2.10 ms | ok |\n\
         | `screenshot` PNG at 120x40 | ≤ 40 ms | 6.30 ms | 6.30 ms | ok |\n\
         | `press --wait <text>` after the text appears | none | 0.35 ms | 0.35 ms | ok |\n\
         | idle daemon memory | ≤ 10 MB | 4.90 MB | 4.90 MB | ok |\n\
         | memory per active tab | about 5 MB (reference) | 9.00 MB | 10.80 MB | over reference, regressed |\n\
         \n\
         - memory per active tab regressed: 10.80 MB against 9.00 MB on the base (+20%; the limit is +10% and +0.25 MB)\n"
    );
    assert_eq!(checked.code, 2, "stderr: {}", checked.stderr);
}

#[test]
fn a_median_over_its_limit_fails_while_memory_per_tab_is_only_over_its_reference() {
    let checked = check(
        r#"{"metrics": [
            {"metric": "round-trip", "base": [2.2], "change": [2.2]},
            {"metric": "snapshot", "base": [14.5], "change": [15.0]},
            {"metric": "png", "base": [6.3], "change": [6.3]},
            {"metric": "press-wait-text", "base": [0.35], "change": [0.35]},
            {"metric": "idle-memory", "base": [4.9], "change": [4.9]},
            {"metric": "tab-memory", "base": [9.0], "change": [9.0]}
        ]}"#,
    )
    .expect("run at-perf check");

    assert_eq!(
        checked.stdout,
        "| metric | target | base | this change | verdict |\n\
         |---|---|---|---|---|\n\
         | CLI round trip (`get frame`) | ≤ 5 ms | 2.20 ms | 2.20 ms | ok |\n\
         | `snapshot` | ≤ 10 ms | 14.50 ms | 15.00 ms | over target |\n\
         | `screenshot` PNG at 120x40 | ≤ 40 ms | 6.30 ms | 6.30 ms | ok |\n\
         | `press --wait <text>` after the text appears | none | 0.35 ms | 0.35 ms | ok |\n\
         | idle daemon memory | ≤ 10 MB | 4.90 MB | 4.90 MB | ok |\n\
         | memory per active tab | about 5 MB (reference) | 9.00 MB | 9.00 MB | over reference |\n\
         \n\
         - `snapshot` is 15.00 ms, over its 10 ms target\n"
    );
    assert_eq!(checked.code, 2, "stderr: {}", checked.stderr);
}

#[test]
fn summary_gets_the_report_appended() {
    let dir = tempfile::tempdir().expect("temp dir");
    let results = dir.path().join("results.json");
    let summary = dir.path().join("summary.md");
    std::fs::write(&summary, "# Earlier step\n").expect("write the summary");
    std::fs::write(
        &results,
        r#"{"metrics": [
            {"metric": "round-trip", "base": [2.0], "change": [2.0]},
            {"metric": "snapshot", "base": [2.0], "change": [2.0]},
            {"metric": "png", "base": [6.0], "change": [6.0]},
            {"metric": "press-wait-text", "base": [0.4], "change": [0.4]},
            {"metric": "idle-memory", "base": [4.85], "change": [4.85]},
            {"metric": "tab-memory", "base": [6.0], "change": [6.0]}
        ]}"#,
    )
    .expect("write the results");

    let output = Command::new(env!("CARGO_BIN_EXE_at-perf"))
        .arg("check")
        .arg(&results)
        .arg("--summary")
        .arg(&summary)
        .output()
        .expect("run at-perf");

    let report = "| metric | target | base | this change | verdict |\n\
                  |---|---|---|---|---|\n\
                  | CLI round trip (`get frame`) | ≤ 5 ms | 2.00 ms | 2.00 ms | ok |\n\
                  | `snapshot` | ≤ 10 ms | 2.00 ms | 2.00 ms | ok |\n\
                  | `screenshot` PNG at 120x40 | ≤ 40 ms | 6.00 ms | 6.00 ms | ok |\n\
                  | `press --wait <text>` after the text appears | none | 0.40 ms | 0.40 ms | ok |\n\
                  | idle daemon memory | ≤ 10 MB | 4.85 MB | 4.85 MB | ok |\n\
                  | memory per active tab | about 5 MB (reference) | 6.00 MB | 6.00 MB | over reference |\n";
    assert_eq!(String::from_utf8_lossy(&output.stdout), report);
    assert_eq!(std::fs::read_to_string(&summary).expect("read the summary"), format!("# Earlier step\n{report}"));
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn results_without_a_metric_are_an_error() {
    let checked = check(
        r#"{"metrics": [
            {"metric": "round-trip", "base": [2.0], "change": [2.0]},
            {"metric": "snapshot", "base": [2.0], "change": [2.0]},
            {"metric": "png", "base": [6.0], "change": [6.0]},
            {"metric": "idle-memory", "base": [4.85], "change": [4.85]},
            {"metric": "tab-memory", "base": [3.0], "change": [3.0]}
        ]}"#,
    )
    .expect("run at-perf check");

    assert_eq!(checked.stdout, "");
    assert_eq!(checked.stderr, "error: the results have no samples of `press --wait <text>` after the text appears\n");
    assert_eq!(checked.code, 1);
}

#[test]
fn a_metric_the_base_could_not_measure_is_held_to_its_limit_and_shows_no_base() {
    let checked = check(
        r#"{"metrics": [
            {"metric": "round-trip", "base": [3.0], "change": [3.0]},
            {"metric": "snapshot", "base": [3.0], "change": [3.0]},
            {"metric": "png", "base": [], "no_base": "the base has no `screenshot`", "change": [50.0]},
            {"metric": "press-wait-text", "base": [], "no_base": "the base has no `press`", "change": [0.4]},
            {"metric": "idle-memory", "base": [4.0], "change": [4.0]},
            {"metric": "tab-memory", "base": [], "no_base": "the base has no `tab new`", "change": [9.0]}
        ]}"#,
    )
    .expect("run at-perf check");

    assert_eq!(
        checked.stdout,
        "| metric | target | base | this change | verdict |\n\
         |---|---|---|---|---|\n\
         | CLI round trip (`get frame`) | ≤ 5 ms | 3.00 ms | 3.00 ms | ok |\n\
         | `snapshot` | ≤ 10 ms | 3.00 ms | 3.00 ms | ok |\n\
         | `screenshot` PNG at 120x40 | ≤ 40 ms | no base | 50.00 ms | over target |\n\
         | `press --wait <text>` after the text appears | none | no base | 0.40 ms | ok |\n\
         | idle daemon memory | ≤ 10 MB | 4.00 MB | 4.00 MB | ok |\n\
         | memory per active tab | about 5 MB (reference) | no base | 9.00 MB | over reference |\n\
         \n\
         - `screenshot` PNG at 120x40 is 50.00 ms, over its 40 ms target\n\
         \n\
         No base to compare with, so no regression check:\n\
         - `screenshot` PNG at 120x40: the base has no `screenshot`\n\
         - `press --wait <text>` after the text appears: the base has no `press`\n\
         - memory per active tab: the base has no `tab new`\n"
    );
    assert_eq!(checked.code, 2, "stderr: {}", checked.stderr);
}

const PASSING: &str = r#"{"metrics": [
    {"metric": "round-trip", "base": [2.0], "change": [2.0]},
    {"metric": "snapshot", "base": [2.0], "change": [2.0]},
    {"metric": "png", "base": [6.0], "change": [6.0]},
    {"metric": "press-wait-text", "base": [0.4], "change": [0.4]},
    {"metric": "idle-memory", "base": [4.85], "change": [4.85]},
    {"metric": "tab-memory", "base": [4.0], "change": [4.0]}
]}"#;

#[test]
fn a_report_that_cannot_be_printed_or_appended_is_an_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let results = dir.path().join("results.json");
    std::fs::write(&results, PASSING).expect("write the results");
    let (reader, writer) = std::io::pipe().expect("a pipe");
    drop(reader);
    let unread = Command::new(env!("CARGO_BIN_EXE_at-perf"))
        .arg("check")
        .arg(&results)
        .stdout(writer)
        .output()
        .expect("run at-perf");
    let not_a_file = Command::new(env!("CARGO_BIN_EXE_at-perf"))
        .arg("check")
        .arg(&results)
        .arg("--summary")
        .arg(dir.path())
        .output()
        .expect("run at-perf");
    assert_eq!([unread.status.code(), not_a_file.status.code()], [Some(1), Some(1)]);
    assert!(String::from_utf8_lossy(&unread.stderr).starts_with("error: print the report: "), "{unread:?}");
    assert!(
        String::from_utf8_lossy(&not_a_file.stderr)
            .starts_with(&format!("error: append the report to {}: ", dir.path().display())),
        "{not_a_file:?}"
    );
}
