use crate::scratch::{Scratch, run};

const FIX: &str = "write the expression at its one call site, or keep the helper for a second caller that needs it → GAUNTLET.md#trivial-helpers";

const CODE: &str = "struct Flags(u8);
impl Flags {
    fn bold(&self) -> bool { self.0 & 1 != 0 }
    fn twice(&self) -> bool { self.bold() && self.bold() }
    pub fn shown(&self) -> bool { self.twice() }
}
impl Clone for Flags {
    fn clone(&self) -> Self { Flags(self.0) }
}
fn branchy(a: u8) -> u8 { if a > 1 { 1 } else { 2 } }
fn tried(a: &str) -> Option<u8> { Some(a.parse::<u8>().ok()?) }
fn steps(a: u8) -> u8 { let b = a; b }
fn half(a: u8) -> u8 { a / 2 }
fn matched(a: u8) -> u8 { match a { 0 => 1, _ => 2 } }
fn looped() -> u8 { loop { break 1 } }
fn waited(a: u8) { while a > 9 {} }
fn counted(a: u8) { for _ in 0..a {} }
fn main() {
    let flags = Flags(half(branchy(3)) + tried(\"1\").unwrap_or(steps(0)) + matched(0) + looped());
    waited(0);
    counted(0);
    let _ = flags.shown();
    let _ = flags.clone();
}
";

#[test]
fn a_private_one_expression_function_called_once_is_named() {
    let scratch = Scratch::crate_with(&[("main.rs", CODE), ("lib.rs", "")]);
    let found = run(&scratch.root(), &["helpers"]);
    assert_eq!(
        (found.code, found.stdout),
        (
            2,
            format!(
                "app/src/main.rs:4: twice has one call site and its body is one expression: {FIX}\n\
                 app/src/main.rs:13: half has one call site and its body is one expression: {FIX}\n\
                 2 offense(s)\n"
            )
        )
    );
}

#[test]
fn a_helper_called_from_its_tests_too_has_two_call_sites() {
    let scratch =
        Scratch::crate_with(&[("main.rs", "fn half(a: u8) -> u8 { a / 2 }\nfn main() { let _ = half(4); }\n")])
            .with("tests/half.rs", "#[test]\nfn halves() { let _ = half(2); }\n");
    let found = run(&scratch.root(), &["helpers"]);
    assert_eq!(
        (found.code, found.stdout.as_str()),
        (0, "1 one-expression private functions, each called more than once\n")
    );
}

#[test]
fn a_source_that_does_not_parse_is_an_error() {
    let scratch = Scratch::crate_with(&[("main.rs", "fn main( {}\n")]);
    let found = run(&scratch.root(), &["helpers"]);
    assert!(found.stderr.starts_with("error: parse "), "{}", found.stderr);
    assert_eq!(found.code, 1);
}
