use crate::scratch::{Scratch, hits, lcov};

const FIX: &str = "test the uncovered behaviour through the crate's real surface, or delete the branch no surface reaches → GAUNTLET.md#coverage-floor";

#[test]
fn a_crate_at_the_floor_passes_and_shows_its_tally() {
    let scratch = Scratch::crate_with(&[("main.rs", "fn main() {}\n")]);
    let main = scratch.source("main.rs");
    let run = scratch.gauntlet("coverage", &lcov(&[(&main, &hits(1..=10, |line| line != 10))]));
    assert_eq!((run.code, run.stdout.as_str(), run.stderr.as_str()), (0, "app: 9 of 10 lines covered\n", ""));
}

#[test]
fn a_crate_under_the_floor_fails_naming_its_most_uncovered_files() {
    let scratch =
        Scratch::crate_with(&[("main.rs", "mod parse;\nfn main() {}\n"), ("parse.rs", "pub fn parse() {}\n")]);
    let (main, parse) = (scratch.source("main.rs"), scratch.source("parse.rs"));
    let report = lcov(&[(&main, &hits(1..=10, |line| line > 1)), (&parse, &hits(1..=10, |line| line > 2))]);
    let run = scratch.gauntlet("coverage", &report);
    assert_eq!(
        run.stdout,
        format!(
            "app: 17 of 20 lines covered, under the 90% floor; most uncovered: app/src/parse.rs (2), app/src/main.rs (1): {FIX}\n1 offense(s)\n"
        )
    );
    assert_eq!(run.code, 2);
}

#[test]
fn a_source_file_with_functions_that_no_report_measured_is_an_offense() {
    let scratch = Scratch::crate_with(&[
        ("main.rs", "mod kinds;\nmod unused;\nfn main() {}\n"),
        ("kinds.rs", "pub struct Kind;\npub const ALL: [Kind; 1] = [Kind];\n"),
        ("unused.rs", "pub fn unused() {}\n"),
    ]);
    let main = scratch.source("main.rs");
    let run = scratch.gauntlet("coverage", &lcov(&[(&main, &hits(1..=3, |_| true))]));
    assert_eq!(run.stdout, format!("app/src/unused.rs: compiled into no measured binary: {FIX}\n1 offense(s)\n"));
    assert_eq!(run.code, 2);
}

#[test]
fn a_crate_with_nothing_measured_fails() {
    let scratch = Scratch::crate_with(&[("main.rs", "fn main() {}\n")]);
    let run = scratch.gauntlet("coverage", "");
    assert_eq!(
        run.stdout,
        format!(
            "app/src/main.rs: compiled into no measured binary: {FIX}\napp: no line of it was measured: {FIX}\n2 offense(s)\n"
        )
    );
    assert_eq!(run.code, 2);
}
