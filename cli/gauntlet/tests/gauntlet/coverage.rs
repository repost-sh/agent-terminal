use crate::scratch::{Scratch, hits, lcov};

const FIX: &str = "test the uncovered behaviour through the crate's real surface, or through a fake at the library's wrapper when only the operating system reaches it; delete only a branch nothing reaches → GAUNTLET.md#coverage-floor";

#[test]
fn a_crate_at_the_floor_passes_and_shows_its_tally() {
    let scratch = Scratch::crate_with(&[("main.rs", "fn main() {}\n")]);
    let main = scratch.source("main.rs");
    let run = scratch.gauntlet("coverage", &lcov(&[(&main, &hits(1..=10, |line| line != 10))]));
    assert_eq!((run.code, run.stdout.as_str(), run.stderr.as_str()), (0, "app: 9 of 10 lines covered\n", ""));
}

#[test]
fn a_crate_under_the_floor_fails_naming_its_most_uncovered_files() {
    let scratch = Scratch::crate_with(&[
        ("main.rs", "mod parse;\nmod print;\nfn main() {}\n"),
        ("parse.rs", "pub fn parse() {}\n"),
        ("print.rs", "pub fn print() {}\n"),
    ]);
    let (main, parse, print) = (scratch.source("main.rs"), scratch.source("parse.rs"), scratch.source("print.rs"));
    let report = lcov(&[
        (&main, &hits(1..=10, |line| line > 1)),
        (&parse, &hits(1..=10, |line| line > 2)),
        (&print, &hits(1..=2, |_| true)),
    ]);
    let run = scratch.gauntlet("coverage", &report);
    assert_eq!(
        run.stdout,
        format!(
            "app: 19 of 22 lines covered, under the 90% floor; most uncovered: app/src/parse.rs (2), app/src/main.rs (1): {FIX}\n1 offense(s)\n"
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

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config { cases: 24, failure_persistence: None, ..proptest::test_runner::Config::default() })]

    #[test]
    fn the_tally_counts_every_measured_line_and_the_floor_is_ninety_percent(covered in proptest::collection::vec(proptest::prelude::any::<bool>(), 1..60)) {
        let scratch = Scratch::crate_with(&[("main.rs", "fn main() {}\n")]);
        let main = scratch.source("main.rs");
        let lines: Vec<(u32, u64)> = (1..).zip(&covered).map(|(line, &hit)| (line, u64::from(hit))).collect();
        let run = scratch.gauntlet("coverage", &lcov(&[(&main, &lines)]));
        let (hit, total) = (covered.iter().filter(|&&hit| hit).count(), covered.len());
        let tally = format!("app: {hit} of {total} lines covered");
        if hit * 100 >= total * 90 {
            proptest::prop_assert_eq!((run.code, run.stdout), (0, format!("{tally}\n")));
        } else {
            proptest::prop_assert!(run.stdout.starts_with(&format!("{tally}, under the 90% floor; most uncovered: app/src/main.rs ({})", total - hit)), "{}", run.stdout);
            proptest::prop_assert_eq!(run.code, 2);
        }
    }
}

#[test]
fn the_unit_tests_beside_a_module_do_not_count_towards_its_crate() {
    let scratch = Scratch::crate_with(&[
        ("main.rs", "fn main() {}\n#[cfg(test)]\nmod tests;\n"),
        ("tests.rs", "#[test]\nfn t() {}\n"),
    ]);
    let (main, tests) = (scratch.source("main.rs"), scratch.source("tests.rs"));
    let run = scratch
        .gauntlet("coverage", &lcov(&[(&main, &hits(1..=10, |line| line < 9)), (&tests, &hits(1..=20, |_| true))]));
    assert_eq!(
        (run.code, run.stdout),
        (
            2,
            format!(
                "app: 8 of 10 lines covered, under the 90% floor; most uncovered: app/src/main.rs (2): {FIX}\n1 offense(s)\n"
            )
        )
    );
}

#[test]
fn a_member_inside_another_is_counted_as_its_own_crate() {
    let scratch = Scratch::crate_with(&[("main.rs", "fn main() {}\n")]).member("app/src/inner", "");
    std::fs::write(scratch.path("src/inner/src/lib.rs"), "pub fn inner() {}\n").expect("write the inner crate");
    let (main, inner) = (scratch.source("main.rs"), scratch.source("inner/src/lib.rs"));
    let run =
        scratch.gauntlet("coverage", &lcov(&[(&main, &hits(1..=10, |_| true)), (&inner, &hits(1..=1, |_| true))]));
    assert_eq!((run.code, run.stdout.as_str()), (0, "app: 10 of 10 lines covered\ninner: 1 of 1 lines covered\n"));
}

#[test]
fn a_source_that_links_nowhere_is_an_error_naming_it() {
    let scratch = Scratch::crate_with(&[("main.rs", "fn main() {}\n"), ("deep/real.rs", "pub fn real() {}\n")]);
    let broken = scratch.path("src/deep/broken.rs");
    std::os::unix::fs::symlink(scratch.path("src/deep/missing.rs"), &broken).expect("link to nothing");
    let run = scratch.gauntlet("coverage", "");
    let named = scratch.source("deep").join("broken.rs");
    assert!(run.stderr.contains(&format!("find where {} is: ", named.display())), "{}", run.stderr);
    assert_eq!(run.code, 1);
}
