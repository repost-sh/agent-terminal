use crate::scratch::{Scratch, hits, lcov};

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
    let run = scratch.gauntlet("crap", &lcov(&[(&main, &hits(1..=10, |line| line <= 5))]));
    assert_eq!((run.code, run.stdout.as_str()), (0, "1 functions at CRAP 6 or under\n"));
}

#[test]
fn less_covered_it_goes_over_the_ceiling() {
    let scratch = Scratch::crate_with(&[("main.rs", BRANCHY)]);
    let main = scratch.source("main.rs");
    let run = scratch.gauntlet("crap", &lcov(&[(&main, &hits(1..=10, |line| line <= 4))]));
    assert_eq!(
        run.stdout,
        format!(
            "app/src/main.rs:1: main has CRAP 7.5 (cyclomatic 4, 40% of its lines covered), over 6: {FIX}\n1 offense(s)\n"
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
    let run = scratch.gauntlet("crap", "");
    let offenses: Vec<String> = ["ifs", "whiles", "fors", "matches", "guards", "logic", "lets", "closures"]
        .iter()
        .zip(1..)
        .map(|(name, line)| {
            format!(
                "app/src/main.rs:{line}: {name} has CRAP 12.0 (cyclomatic 3, 0% of its lines covered), over 6: {FIX}"
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
    let run = scratch.gauntlet("crap", &lcov(&[(&main, &hits(1..=10, |line| !(2..=4).contains(&line)))]));
    assert_eq!(
        run.stdout,
        format!(
            "app/src/main.rs:2: inner has CRAP 12.0 (cyclomatic 3, 0% of its lines covered), over 6: {FIX}\n1 offense(s)\n"
        )
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
    let run = scratch.gauntlet("crap", "");
    assert_eq!(
        run.stdout,
        format!(
            "app/src/main.rs:3: Area::area has CRAP 12.0 (cyclomatic 3, 0% of its lines covered), over 6: {FIX}\n\
             app/src/main.rs:6: Shape::sides has CRAP 12.0 (cyclomatic 3, 0% of its lines covered), over 6: {FIX}\n\
             2 offense(s)\n"
        )
    );
}

#[test]
fn a_source_file_that_does_not_parse_is_an_error() {
    let scratch = Scratch::crate_with(&[("main.rs", "fn main( {}\n")]);
    let run = scratch.gauntlet("crap", "");
    let main = scratch.source("main.rs");
    assert!(run.stderr.starts_with(&format!("error: parse {}: ", main.display())), "{}", run.stderr);
    assert_eq!((run.code, run.stdout.as_str()), (1, ""));
}
