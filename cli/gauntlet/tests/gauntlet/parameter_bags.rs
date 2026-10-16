use crate::scratch::{Scratch, run};

const FIX: &str = "pass what the function needs itself, or give the struct the methods that use its fields → GAUNTLET.md#parameter-bags";

const CODE: &str = "#[derive(serde::Serialize)]
struct Wire { a: u8 }
struct Missed { a: u8, b: u8 }
struct Shared { a: u8 }
struct Point { x: u8 }
struct Twice { a: u8 }
struct Tuple(u8);
impl Point {
    fn sum(&self) -> u8 { self.x }
}
impl Default for Shared {
    fn default() -> Self { Self { a: 0 } }
}
fn failed(missed: &Missed) -> u8 { missed.a + missed.b }
fn first(shared: &Shared) -> u8 { shared.a }
fn second(shared: Option<&Shared>) -> u8 { shared.map_or(0, |shared| shared.a) }
fn send(wire: Wire) -> u8 { wire.a }
fn main() {
    let total = failed(&Missed { a: 1, b: 2 }) + first(&Shared::default()) + second(None);
    let _ = total + send(Wire { a: 1 }) + Point { x: 1 }.sum() + Twice { a: 1 }.a + Twice { a: 2 }.a + Tuple(1).0;
}
";

#[test]
fn a_struct_built_once_to_be_taken_by_one_function_is_named() {
    let scratch = Scratch::crate_with(&[("main.rs", CODE)]);
    let found = run(&scratch.root(), &["bags"]);
    assert_eq!(
        (found.code, found.stdout),
        (
            2,
            format!(
                "app/src/main.rs:3: Missed is built in one place and taken by one function: {FIX}\n\
                 app/src/main.rs:5: Point is built in one place and taken by one function: {FIX}\n\
                 2 offense(s)\n"
            )
        )
    );
}

#[test]
fn a_struct_its_tests_build_too_is_built_in_two_places() {
    let scratch = Scratch::crate_with(&[("main.rs", "struct Missed { a: u8 }\nfn failed(missed: &Missed) -> u8 { missed.a }\nfn main() { let _ = failed(&Missed { a: 1 }); }\n")])
        .with("tests/missed.rs", "#[test]\nfn built() { let _ = Missed { a: 2 }; }\n");
    let found = run(&scratch.root(), &["bags"]);
    assert_eq!(
        (found.code, found.stdout.as_str()),
        (0, "1 structs with named fields, none built once for one function\n")
    );
}

#[test]
fn a_source_that_does_not_parse_is_an_error() {
    let scratch = Scratch::crate_with(&[("main.rs", "fn main( {}\n")]);
    let found = run(&scratch.root(), &["bags"]);
    assert!(found.stderr.starts_with("error: parse "), "{}", found.stderr);
    assert_eq!(found.code, 1);
}
