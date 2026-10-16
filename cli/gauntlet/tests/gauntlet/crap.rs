use crate::scratch::{Scratch, hits, regions};

const FIX: &str = "cover every branch through the crate's real surface, or split it into simpler functions that are each covered → GAUNTLET.md#crap";

const BRANCHY: &str = "fn main() {
    let value = std::env::args().count();
    if value > 1 && value < 5 {
        std::process::abort();
    }
    match value {
        0 => std::process::abort(),
        _ => std::process::abort(),
    }
}
";

#[test]
fn half_covered_cyclomatic_four_is_exactly_at_the_ceiling() {
    let scratch = Scratch::crate_with(&[("main.rs", BRANCHY)]);
    let main = scratch.source("main.rs");
    let run = scratch.gauntlet("crap", &regions(&[(&main, &hits(1..=10, |line| line <= 5))]));
    assert_eq!((run.code, run.stdout.as_str()), (0, "1 functions at CRAP 6 or under\n"));
}

#[test]
fn less_covered_it_goes_over_the_ceiling() {
    let scratch = Scratch::crate_with(&[("main.rs", BRANCHY)]);
    let main = scratch.source("main.rs");
    let run = scratch.gauntlet("crap", &regions(&[(&main, &hits(1..=10, |line| line <= 4))]));
    assert_eq!(
        run.stdout,
        format!(
            "app/src/main.rs:1: main has CRAP 7.5 (cyclomatic 4, 40% of its regions covered), over 6: {FIX}\n1 offense(s)\n"
        )
    );
    assert_eq!(run.code, 2);
}

const CONSTRUCTS: &str = "fn ifs(a: bool) { if a {} if a {} }
fn whiles(a: bool) { while a {} while a {} }
fn fors() { for _ in 0..1 {} for _ in 0..1 {} }
fn matches(a: u8) { match a { 0 => {} _ => {} } match a { 1 => {} 2 => {} 3 => {} _ => {} } }
fn guards(a: u8) { match a { b if b > 1 => {} _ => {} } }
fn logic(a: bool, b: bool) -> bool { a && b || a }
fn lets(a: Option<u8>) { let Some(_) = a else { return }; let Some(_) = a else { return }; }
fn closures(a: bool) { let f = |b: bool| if b {}; if a {} f(a); }
fn loops() { loop { break } }
fn main() {}
";

#[test]
fn each_branching_construct_adds_one_path() {
    let scratch = Scratch::crate_with(&[("main.rs", CONSTRUCTS)]);
    let run = scratch.gauntlet("crap", &regions(&[]));
    let offenses: Vec<String> = ["ifs", "whiles", "fors", "matches", "guards", "logic", "lets", "closures"]
        .iter()
        .zip(1..)
        .map(|(name, line)| {
            format!(
                "app/src/main.rs:{line}: {name} has CRAP 12.0 (cyclomatic 3, 0% of its regions covered), over 6: {FIX}"
            )
        })
        .chain(["8 offense(s)".to_string()])
        .collect();
    assert_eq!(run.stdout.lines().collect::<Vec<_>>(), offenses);
    assert_eq!(run.code, 2);
}

const NESTED: &str = "fn main() {
    fn inner(a: bool) -> bool {
        a && a || a
    }
    if inner(true) {}
    if inner(false) {}
    if inner(true) {}
    if inner(false) {}
    if inner(true) {}
}
";

#[test]
fn a_nested_function_is_scored_apart_from_the_function_around_it() {
    let scratch = Scratch::crate_with(&[("main.rs", NESTED)]);
    let main = scratch.source("main.rs");
    let run = scratch.gauntlet("crap", &regions(&[(&main, &hits(1..=10, |line| !(2..=4).contains(&line)))]));
    assert_eq!(
        run.stdout,
        format!("app/src/main.rs:2: inner ran under no test: none of its 3 regions ran: {FIX}\n1 offense(s)\n")
    );
}

const OWNED: &str = "struct Shape;
trait Area {
    fn area(&self) -> u8 { if true { 1 } else if false { 2 } else { 3 } }
}
impl Shape {
    fn sides(&self) -> u8 { if true { 1 } else if false { 2 } else { 3 } }
}
impl Area for Shape {}
fn main() {}
";

#[test]
fn methods_are_named_for_the_type_or_trait_that_owns_them() {
    let scratch = Scratch::crate_with(&[("main.rs", OWNED)]);
    let run = scratch.gauntlet("crap", &regions(&[]));
    assert_eq!(
        run.stdout,
        format!(
            "app/src/main.rs:3: Area::area has CRAP 12.0 (cyclomatic 3, 0% of its regions covered), over 6: {FIX}\n\
             app/src/main.rs:6: Shape::sides has CRAP 12.0 (cyclomatic 3, 0% of its regions covered), over 6: {FIX}\n\
             2 offense(s)\n"
        )
    );
}

#[test]
fn a_source_file_that_does_not_parse_is_an_error() {
    let scratch = Scratch::crate_with(&[("main.rs", "fn main( {}\n")]);
    let run = scratch.gauntlet("crap", &regions(&[]));
    let main = scratch.source("main.rs");
    assert!(run.stderr.starts_with(&format!("error: parse {}: ", main.display())), "{}", run.stderr);
    assert_eq!((run.code, run.stdout.as_str()), (1, ""));
}

const DISPATCH: &str = "fn main() {
    match std::env::args().count() {
        0 => std::process::abort(),
        1 => std::process::exit(1),
        2 => std::process::exit(2),
        _ => std::process::exit(3),
    }
}
";

#[test]
fn a_function_no_test_runs_fails_however_simple_it_is() {
    let scratch = Scratch::crate_with(&[("main.rs", DISPATCH)]);
    let main = scratch.source("main.rs");
    let run = scratch.gauntlet("crap", &regions(&[(&main, &hits(1..=8, |_| false))]));
    assert_eq!(
        (run.code, run.stdout),
        (2, format!("app/src/main.rs:1: main ran under no test: none of its 8 regions ran: {FIX}\n1 offense(s)\n"))
    );
}

#[test]
fn an_arm_that_never_ran_counts_against_its_function_even_on_a_line_that_did() {
    let scratch = Scratch::crate_with(&[("main.rs", BRANCHY)]);
    let main = scratch.source("main.rs");
    let mut counted = hits(1..=10, |line| line <= 5);
    counted.push((2, 0));
    let run = scratch.gauntlet("crap", &regions(&[(&main, &counted)]));
    assert_eq!(
        run.stdout,
        format!(
            "app/src/main.rs:1: main has CRAP 6.6 (cyclomatic 4, 45% of its regions covered), over 6: {FIX}\n1 offense(s)\n"
        )
    );
}

#[test]
fn a_coverage_export_that_does_not_parse_is_an_error() {
    let scratch = Scratch::crate_with(&[("main.rs", "fn main() {}\n")]);
    let run = scratch.gauntlet("crap", "{\"data\": [");
    assert!(run.stderr.starts_with("error: parse ") && run.stderr.contains("EOF while parsing"), "{}", run.stderr);
    assert_eq!(run.code, 1);
}

#[test]
fn only_code_regions_of_a_file_the_export_names_count() {
    let scratch = Scratch::crate_with(&[("main.rs", "fn main() {\n    std::process::abort();\n}\n")]);
    let main = scratch.source("main.rs");
    let export = serde_json::json!({ "data": [{ "functions": [
        { "filenames": [&main], "regions": [[1, 1, 1, 2, 0, 0, 0, 0], [2, 1, 2, 2, 5, 0, 0, 3], [2, 1, 2, 2, 5, 1, 0, 0]] },
        { "filenames": [&main], "regions": [[1, 1, 1, 2, 3, 0, 0, 0]] }
    ] }] });
    let run = scratch.gauntlet("crap", &export.to_string());
    assert_eq!((run.code, run.stdout.as_str()), (0, "1 functions at CRAP 6 or under\n"));
    let unrun = serde_json::json!({ "data": [{ "functions": [
        { "filenames": [&main], "regions": [[1, 1, 1, 2, 0, 0, 0, 0], [2, 1, 2, 2, 5, 0, 0, 3], [2, 1, 2, 2, 5, 1, 0, 0]] }
    ] }] });
    let run = scratch.gauntlet("crap", &unrun.to_string());
    assert_eq!(
        run.stdout,
        format!("app/src/main.rs:1: main ran under no test: none of its 1 regions ran: {FIX}\n1 offense(s)\n")
    );
}
