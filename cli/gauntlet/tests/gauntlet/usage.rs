use crate::scratch::{Scratch, run};

const USAGE: &str = "usage: at-gauntlet coverage <lcov.info>\n       at-gauntlet crap <lcov.info>";

#[test]
fn without_a_gate_and_a_report_it_prints_the_usage() {
    let dir = tempfile::tempdir().expect("scratch folder");
    for args in [&[][..], &["coverage"], &["coverage", "a", "b"]] {
        let refused = run(dir.path(), args);
        assert_eq!((refused.code, refused.stderr), (1, format!("error: {USAGE}\n")), "{args:?}");
    }
}

#[test]
fn an_unknown_gate_is_refused_before_the_report_is_read() {
    let dir = tempfile::tempdir().expect("scratch folder");
    let refused = run(dir.path(), &["mutation", "missing.info"]);
    assert_eq!((refused.code, refused.stderr), (1, format!("error: unknown gate mutation\n{USAGE}\n")));
}

#[test]
fn a_report_that_cannot_be_read_is_an_error() {
    let dir = tempfile::tempdir().expect("scratch folder");
    let refused = run(dir.path(), &["coverage", "missing.info"]);
    assert_eq!(
        (refused.code, refused.stderr),
        (1, "error: read missing.info: No such file or directory (os error 2)\n".to_string())
    );
}

#[test]
fn a_report_with_a_broken_line_names_it() {
    let scratch = Scratch::crate_with(&[("main.rs", "fn main() {}\n")]);
    for (report, problem) in [
        ("DA:1,1\n", "line 1: DA before any SF"),
        ("SF:main.rs\nDA:x,1\n", "line 2: DA:x,1: a line number: invalid digit found in string"),
        ("SF:main.rs\nDA:1\n", "line 2: DA:1: a hit count"),
        ("SF:main.rs\nDA:1,-1\n", "line 2: DA:1,-1: a hit count: invalid digit found in string"),
    ] {
        let refused = scratch.gauntlet("coverage", report);
        assert!(refused.stderr.ends_with(&format!("lcov.info: {problem}\n")), "{}", refused.stderr);
        assert_eq!(refused.code, 1);
    }
}

#[test]
fn outside_a_cargo_workspace_it_says_cargo_metadata_failed() {
    let dir = tempfile::tempdir().expect("scratch folder");
    std::fs::write(dir.path().join("lcov.info"), "").expect("write the report");
    let refused = run(dir.path(), &["coverage", "lcov.info"]);
    assert!(
        refused.stderr.starts_with("error: read the workspace with cargo metadata: error: could not find `Cargo.toml`"),
        "{}",
        refused.stderr
    );
    assert_eq!(refused.code, 1);
}
