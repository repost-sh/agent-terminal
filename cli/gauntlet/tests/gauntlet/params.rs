use crate::scratch::{Run, Scratch};

const FIX: &str = "group the parameters into one struct with named fields, or keep the function inside its module → GAUNTLET.md#parameter-ceiling";

fn params(code: &str) -> Run {
    Scratch::crate_with(&[("main.rs", code)]).params()
}

fn over(file: &str, line: usize, function: &str, count: usize) -> String {
    format!("app/{file}:{line}: {function} takes {count} parameters: {FIX}\n1 offense(s)\n")
}

#[test]
fn functions_kept_inside_their_module_may_take_more() {
    let code = "fn main() {}

fn private(a: u8, b: u8, c: u8, d: u8) {}

struct Shape;

impl std::ops::Add<u8> for Shape {
    type Output = u8;

    fn add(self, other: u8) -> u8 {
        other
    }
}

trait Hidden {
    fn hidden(&self, a: u8, b: u8, c: u8);
}

pub fn two(a: u8, b: u8) {}

impl Shape {
    pub fn method(&self, a: u8, b: u8) {}

    pub fn arced(self: &std::sync::Arc<Self>, a: u8, b: u8) {}
}
";
    let run = params(code);
    assert_eq!(
        (run.code, run.stdout.as_str(), run.stderr.as_str()),
        (0, "3 functions shared beyond their module take at most 2 parameters\n", "")
    );
}

#[test]
fn every_kind_of_shared_function_over_the_ceiling_fails() {
    for (code, function) in [
        ("pub fn wide(a: u8, b: u8, c: u8) {}\n", "wide"),
        ("pub(crate) fn wide(a: u8, b: u8, c: u8) {}\n", "wide"),
        ("mod inner {\n    pub(super) fn wide(a: u8, b: u8, c: u8) {}\n}\n", "wide"),
        ("mod inner {\n    pub(in crate::inner) fn wide(a: u8, b: u8, c: u8) {}\n}\n", "wide"),
        ("struct Shape;\n\nimpl Shape {\n    pub(crate) fn wide(&self, a: u8, b: u8, c: u8) {}\n}\n", "Shape::wide"),
        (
            "struct Boxed<T>(T);\n\nimpl<T> Boxed<T> {\n    pub fn wide(&self, a: u8, b: u8, c: u8) {}\n}\n",
            "Boxed::wide",
        ),
        ("trait Shape {}\n\nimpl dyn Shape {\n    pub fn wide(&self, a: u8, b: u8, c: u8) {}\n}\n", "dyn Shape::wide"),
        ("pub trait Shape {\n    fn wide(&self, a: u8, b: u8, c: u8);\n}\n", "Shape::wide"),
        ("fn outer() {\n    pub fn wide(a: u8, b: u8, c: u8) {}\n}\n", "wide"),
    ] {
        let line = code.lines().position(|line| line.contains("fn wide")).expect("a wide function") + 2;
        let run = params(&format!("fn main() {{}}\n{code}"));
        assert_eq!((run.code, run.stdout), (2, over("src/main.rs", line, function, 3)), "{code}");
    }
}

#[test]
fn a_helper_in_the_crates_tests_counts_too() {
    let run = Scratch::crate_with(&[("main.rs", "fn main() {}\n")])
        .with("tests/helpers.rs", "pub fn helper(a: u8, b: u8, c: u8, d: u8) {}\n")
        .params();
    assert_eq!((run.code, run.stdout), (2, over("tests/helpers.rs", 1, "helper", 4)));
}

#[test]
fn a_file_that_does_not_parse_is_an_error_naming_it() {
    let run = params("fn (\n");
    assert!(run.stderr.starts_with("error: parse "), "{}", run.stderr);
    assert!(run.stderr.contains("app/src/main.rs: "), "{}", run.stderr);
    assert_eq!(run.code, 1);
}
