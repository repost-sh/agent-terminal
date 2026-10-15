use crate::offenses::{Offenses, Rule};
use crate::repo_files::{cli, read, relative, rust_files};
use crate::rust_source::scan;

enum Scope {
    Everywhere,
    ProductCode,
    OutsideOf(&'static str),
    OutsideOfAny(&'static [&'static str]),
}

impl Scope {
    fn applies_to(&self, relative: &str) -> bool {
        match self {
            Scope::Everywhere => true,
            Scope::ProductCode => relative.starts_with("src/"),
            Scope::OutsideOf(home) => !relative.starts_with(home),
            Scope::OutsideOfAny(homes) => !homes.iter().any(|home| relative.starts_with(home)),
        }
    }
}

const BANNED: &[(&str, &str, Scope)] = &[
    ("unsafe", "unsafe code", Scope::Everywhere),
    ("todo!", "todo! placeholder", Scope::Everywhere),
    ("unimplemented!", "unimplemented! placeholder", Scope::Everywhere),
    ("dbg!", "dbg! left in", Scope::Everywhere),
    ("#[allow(", "silenced lint", Scope::Everywhere),
    ("#![allow(", "silenced lint", Scope::Everywhere),
    ("#[expect(", "silenced lint", Scope::Everywhere),
    ("#![expect(", "silenced lint", Scope::Everywhere),
    ("#[ignore", "skipped test", Scope::Everywhere),
    ("thread::sleep", "sleep used as synchronization; wait on the event instead", Scope::ProductCode),
    ("let _ =", "dropped result; return it or log why it can fail", Scope::ProductCode),
    ("alacritty_terminal", "the screen engine belongs in src/native/screen/", Scope::OutsideOf("src/native/screen/")),
    ("portable_pty", "the PTY library belongs in src/native/pty.rs", Scope::OutsideOf("src/native/pty.rs")),
    (
        "num_traits",
        "numeric conversions for drawing belong in src/native/render/",
        Scope::OutsideOf("src/native/render/"),
    ),
    (
        "tracing",
        "the log library belongs in src/native/log.rs and perf/src/log.rs",
        Scope::OutsideOfAny(&["src/native/log.rs", "perf/src/log.rs"]),
    ),
    (
        "tracing_subscriber",
        "the log library belongs in src/native/log.rs and perf/src/log.rs",
        Scope::OutsideOfAny(&["src/native/log.rs", "perf/src/log.rs"]),
    ),
    (
        "toml",
        "the TOML reader belongs in tests/static_checks/lint_table.rs",
        Scope::OutsideOf("tests/static_checks/lint_table.rs"),
    ),
];

const FIX: &str = "fix the code the token stands for: handle the error, wait on the event, move the library call \
                   into its home module, or write the missing code";

#[test]
fn rust_files_use_no_banned_token_and_each_library_only_in_its_home() {
    let mut offenses = Offenses::of(Rule { anchor: "banned-tokens", fix: FIX });
    for file in rust_files() {
        let relative = relative(&file, cli());
        let code = scan(&read(&file)).code;
        for (pattern, reason, _) in BANNED.iter().filter(|(_, _, scope)| scope.applies_to(&relative)) {
            for (index, _) in code.lines().enumerate().filter(|(_, line)| contains_token(line, pattern)) {
                offenses.add(format!("{relative}:{}: {reason} (`{pattern}`)", index + 1));
            }
        }
        for line in silenced_through_cfg_attr(&code) {
            offenses.add(format!("{relative}:{line}: silenced lint (`cfg_attr` around `allow` or `expect`)"));
        }
    }
    offenses.assert_none();
}

fn silenced_through_cfg_attr(code: &str) -> Vec<usize> {
    let attribute = regex::Regex::new(r"#!?\[\s*cfg_attr\s*\(([^\]]*)\]").expect("the cfg_attr pattern");
    attribute
        .captures_iter(code)
        .filter(|found| {
            found[1].split(|c: char| !c.is_alphanumeric() && c != '_').any(|word| ["allow", "expect"].contains(&word))
        })
        .map(|found| code[..found.get(0).map_or(0, |whole| whole.start())].matches('\n').count() + 1)
        .collect()
}

fn contains_token(line: &str, pattern: &str) -> bool {
    let is_word_char = |c: char| c.is_alphanumeric() || c == '_';
    line.match_indices(pattern).any(|(start, _)| {
        let before = line[..start].chars().next_back();
        let after = line[start + pattern.len()..].chars().next();
        let starts_clean = !pattern.starts_with(is_word_char) || !before.is_some_and(is_word_char);
        let ends_clean = !pattern.ends_with(is_word_char) || !after.is_some_and(is_word_char);
        starts_clean && ends_clean
    })
}

#[test]
fn banned_tokens_match_whole_words_only() {
    assert!(contains_token("    unsafe { x }", "unsafe"));
    assert!(!contains_token("let not_unsafe = 1;", "unsafe"));
    assert!(contains_token("#[allow(dead_code)]", "#[allow("));
    assert!(contains_token("        let _ = writer.flush();", "let _ ="));
    assert!(contains_token("use portable_pty::CommandBuilder;", "portable_pty"));
}

#[test]
fn scopes_limit_where_a_ban_applies() {
    assert!(Scope::ProductCode.applies_to("src/native/terminal.rs"));
    assert!(!Scope::ProductCode.applies_to("tests/fixtures/src/bin/line-printer.rs"));
    assert!(!Scope::OutsideOf("src/native/pty.rs").applies_to("src/native/pty.rs"));
    assert!(Scope::OutsideOf("src/native/pty.rs").applies_to("src/native/daemon.rs"));
}

#[test]
fn a_library_with_a_home_in_each_crate_is_banned_everywhere_else() {
    let log = Scope::OutsideOfAny(&["src/native/log.rs", "perf/src/log.rs"]);
    assert!(!log.applies_to("perf/src/log.rs"));
    assert!(log.applies_to("perf/src/daemon.rs"));
    assert!(!Scope::OutsideOf("src/native/screen/").applies_to("src/native/screen/rows.rs"));
}

#[test]
fn cfg_attr_around_allow_or_expect_is_found_on_its_line() {
    let code =
        "fn a() {}\n#[cfg_attr(test, allow(dead_code))]\nfn b() {}\n#![cfg_attr(feature = \"x\", expect(unused))]\n";
    assert_eq!(silenced_through_cfg_attr(code), [2, 4]);
    assert_eq!(silenced_through_cfg_attr("#[cfg_attr(test, derive(Debug))]\nstruct A;\n"), Vec::<usize>::new());
}
