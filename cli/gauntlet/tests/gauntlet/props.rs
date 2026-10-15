use crate::scratch::Scratch;

const FIX: &str = "state an invariant of the crate's behaviour as a proptest! with a #[test], an input strategy and an assertion → GAUNTLET.md#property-tests";

const PROPERTY: &str = "proptest::proptest! {
    #[test]
    fn doubling_is_even(n in 0u32..1000) {
        proptest::prop_assert_eq!((n * 2) % 2, 0);
    }
}
";

fn offense() -> String {
    format!("app: no property test: {FIX}\n1 offense(s)\n")
}

#[test]
fn a_property_in_the_crates_tests_passes() {
    let scratch = Scratch::crate_with(&[("main.rs", "fn main() {}\n")]).with("tests/doubling.rs", PROPERTY);
    let run = scratch.props();
    assert_eq!((run.code, run.stdout.as_str(), run.stderr.as_str()), (0, "app: has a property test\n", ""));
}

#[test]
fn a_property_in_a_test_module_of_the_source_passes() {
    let code = format!("fn main() {{}}\n#[cfg(test)]\nmod tests {{\n{PROPERTY}}}\n");
    let run = Scratch::crate_with(&[("main.rs", &code)]).props();
    assert_eq!((run.code, run.stdout.as_str()), (0, "app: has a property test\n"));
}

#[test]
fn the_macro_name_alone_is_not_a_property() {
    let without_assertion = PROPERTY.replace("proptest::prop_assert_eq!((n * 2) % 2, 0);", "let _ = n;");
    let without_strategy =
        "proptest::proptest! {\n    #[test]\n    fn fixed() {\n        proptest::prop_assert!(true);\n    }\n}\n";
    let without_test = PROPERTY.replace("#[test]", "");
    let named_in_text = "fn main() { let _ = \"proptest! { #[test] fn x(n in 0..1) { prop_assert!(true) } }\"; }\n";
    let switched_off = format!("#[cfg(feature = \"never\")]\nmod off {{\n{PROPERTY}}}\n");
    let file_switched_off = format!("#![cfg(any())]\n{PROPERTY}");
    for decoy in
        [without_assertion.as_str(), without_strategy, &without_test, named_in_text, &switched_off, &file_switched_off]
    {
        let run = Scratch::crate_with(&[("main.rs", "fn main() {}\n")]).with("tests/decoy.rs", decoy).props();
        assert_eq!((run.code, run.stdout), (2, offense()), "{decoy}");
    }
}

#[test]
fn a_test_file_that_does_not_parse_is_an_error_naming_it() {
    let run = Scratch::crate_with(&[("main.rs", "fn main() {}\n")]).with("tests/broken.rs", "fn (\n").props();
    assert!(run.stderr.starts_with("error: parse "), "{}", run.stderr);
    assert!(run.stderr.contains("tests/broken.rs: "), "{}", run.stderr);
    assert_eq!(run.code, 1);
}
