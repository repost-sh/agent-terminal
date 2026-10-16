use std::path::Path;

use toml::{Table, Value};

use crate::offenses::{Offenses, Rule};
use crate::repo_files::{cli, files_under, read, relative, repo};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Level {
    Allow,
    Warn,
    Deny,
    Forbid,
}

impl Level {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "allow" => Some(Self::Allow),
            "warn" => Some(Self::Warn),
            "deny" => Some(Self::Deny),
            "forbid" => Some(Self::Forbid),
            _ => None,
        }
    }
}

struct Required {
    lint: &'static str,
    level: Level,
    priority: i64,
}

const RUST: &[Required] = &[
    Required { lint: "unsafe_code", level: Level::Forbid, priority: 0 },
    Required { lint: "rust_2018_idioms", level: Level::Deny, priority: -1 },
    Required { lint: "elided_lifetimes_in_paths", level: Level::Deny, priority: 0 },
    Required { lint: "ffi_unwind_calls", level: Level::Deny, priority: 0 },
    Required { lint: "macro_use_extern_crate", level: Level::Deny, priority: 0 },
    Required { lint: "meta_variable_misuse", level: Level::Deny, priority: 0 },
    Required { lint: "non_ascii_idents", level: Level::Deny, priority: 0 },
    Required { lint: "redundant_lifetimes", level: Level::Deny, priority: 0 },
    Required { lint: "trivial_casts", level: Level::Deny, priority: 0 },
    Required { lint: "trivial_numeric_casts", level: Level::Deny, priority: 0 },
    Required { lint: "unit_bindings", level: Level::Deny, priority: 0 },
    Required { lint: "unreachable_pub", level: Level::Deny, priority: 0 },
    Required { lint: "unused_lifetimes", level: Level::Deny, priority: 0 },
    Required { lint: "unused_macro_rules", level: Level::Deny, priority: 0 },
    Required { lint: "unused_qualifications", level: Level::Deny, priority: 0 },
];

const CLIPPY: &[Required] = &[
    Required { lint: "pedantic", level: Level::Warn, priority: -1 },
    Required { lint: "allow_attributes", level: Level::Deny, priority: 0 },
    Required { lint: "allow_attributes_without_reason", level: Level::Deny, priority: 0 },
    Required { lint: "await_holding_lock", level: Level::Deny, priority: 0 },
    Required { lint: "cast_possible_truncation", level: Level::Deny, priority: 0 },
    Required { lint: "cast_possible_wrap", level: Level::Deny, priority: 0 },
    Required { lint: "cast_precision_loss", level: Level::Deny, priority: 0 },
    Required { lint: "cast_sign_loss", level: Level::Deny, priority: 0 },
    Required { lint: "cognitive_complexity", level: Level::Deny, priority: 0 },
    Required { lint: "dbg_macro", level: Level::Deny, priority: 0 },
    Required { lint: "excessive_nesting", level: Level::Deny, priority: 0 },
    Required { lint: "expect_used", level: Level::Deny, priority: 0 },
    Required { lint: "indexing_slicing", level: Level::Deny, priority: 0 },
    Required { lint: "large_enum_variant", level: Level::Deny, priority: 0 },
    Required { lint: "large_futures", level: Level::Deny, priority: 0 },
    Required { lint: "map_err_ignore", level: Level::Deny, priority: 0 },
    Required { lint: "needless_pass_by_value", level: Level::Deny, priority: 0 },
    Required { lint: "panic", level: Level::Deny, priority: 0 },
    Required { lint: "print_stderr", level: Level::Deny, priority: 0 },
    Required { lint: "print_stdout", level: Level::Deny, priority: 0 },
    Required { lint: "redundant_clone", level: Level::Deny, priority: 0 },
    Required { lint: "result_large_err", level: Level::Deny, priority: 0 },
    Required { lint: "should_implement_trait", level: Level::Deny, priority: 0 },
    Required { lint: "todo", level: Level::Deny, priority: 0 },
    Required { lint: "too_many_arguments", level: Level::Deny, priority: 0 },
    Required { lint: "too_many_lines", level: Level::Deny, priority: 0 },
    Required { lint: "type_complexity", level: Level::Deny, priority: 0 },
    Required { lint: "undocumented_unsafe_blocks", level: Level::Deny, priority: 0 },
    Required { lint: "unimplemented", level: Level::Deny, priority: 0 },
    Required { lint: "unreachable", level: Level::Deny, priority: 0 },
    Required { lint: "unused_result_ok", level: Level::Deny, priority: 0 },
    Required { lint: "unwrap_used", level: Level::Deny, priority: 0 },
    Required { lint: "wildcard_imports", level: Level::Deny, priority: 0 },
    Required { lint: "clone_on_ref_ptr", level: Level::Deny, priority: 0 },
    Required { lint: "empty_drop", level: Level::Deny, priority: 0 },
    Required { lint: "empty_enum_variants_with_brackets", level: Level::Deny, priority: 0 },
    Required { lint: "empty_structs_with_brackets", level: Level::Deny, priority: 0 },
    Required { lint: "error_impl_error", level: Level::Deny, priority: 0 },
    Required { lint: "exit", level: Level::Deny, priority: 0 },
    Required { lint: "filetype_is_file", level: Level::Deny, priority: 0 },
    Required { lint: "fn_to_numeric_cast_any", level: Level::Deny, priority: 0 },
    Required { lint: "get_unwrap", level: Level::Deny, priority: 0 },
    Required { lint: "if_then_some_else_none", level: Level::Deny, priority: 0 },
    Required { lint: "infinite_loop", level: Level::Deny, priority: 0 },
    Required { lint: "iter_over_hash_type", level: Level::Deny, priority: 0 },
    Required { lint: "lossy_float_literal", level: Level::Deny, priority: 0 },
    Required { lint: "mem_forget", level: Level::Deny, priority: 0 },
    Required { lint: "mixed_read_write_in_expression", level: Level::Deny, priority: 0 },
    Required { lint: "partial_pub_fields", level: Level::Deny, priority: 0 },
    Required { lint: "pathbuf_init_then_push", level: Level::Deny, priority: 0 },
    Required { lint: "rc_buffer", level: Level::Deny, priority: 0 },
    Required { lint: "rc_mutex", level: Level::Deny, priority: 0 },
    Required { lint: "redundant_type_annotations", level: Level::Deny, priority: 0 },
    Required { lint: "renamed_function_params", level: Level::Deny, priority: 0 },
    Required { lint: "same_name_method", level: Level::Deny, priority: 0 },
    Required { lint: "self_named_module_files", level: Level::Deny, priority: 0 },
    Required { lint: "string_add", level: Level::Deny, priority: 0 },
    Required { lint: "string_lit_chars_any", level: Level::Deny, priority: 0 },
    Required { lint: "suspicious_xor_used_as_pow", level: Level::Deny, priority: 0 },
    Required { lint: "try_err", level: Level::Deny, priority: 0 },
    Required { lint: "unnecessary_safety_comment", level: Level::Deny, priority: 0 },
    Required { lint: "unnecessary_safety_doc", level: Level::Deny, priority: 0 },
    Required { lint: "unnecessary_self_imports", level: Level::Deny, priority: 0 },
    Required { lint: "unneeded_field_pattern", level: Level::Deny, priority: 0 },
    Required { lint: "unwrap_in_result", level: Level::Deny, priority: 0 },
    Required { lint: "verbose_file_reads", level: Level::Deny, priority: 0 },
];

const THRESHOLDS: &[(&str, i64)] = &[
    ("cognitive-complexity-threshold", 6),
    ("too-many-arguments-threshold", 6),
    ("too-many-lines-threshold", 40),
    ("excessive-nesting-threshold", 4),
];

const IN_TESTS: &[&str] = &[
    "allow-unwrap-in-tests",
    "allow-expect-in-tests",
    "allow-panic-in-tests",
    "allow-indexing-slicing-in-tests",
    "allow-print-in-tests",
];

const RENAMED_PARAMS: &[&str] = &[
    "core::convert::From",
    "core::convert::TryFrom",
    "core::str::FromStr",
    "std::fmt::Debug",
    "std::fmt::Display",
    "syn::visit::Visit",
];

const EXIT_AND_ABORT: &[&str] = &["std::process::exit", "std::process::abort"];

const CLIPPY_CONFIGS: &[(&str, &[&str])] = &[
    ("cli/clippy.toml", &["std::process::exit", "std::process::abort", "std::thread::sleep"]),
    ("cli/tests/fixtures/clippy.toml", EXIT_AND_ABORT),
];

const OTHER_KEYS: &[&str] = &["avoid-breaking-exported-api", "allow-renamed-params-for", "disallowed-methods"];

fn sealed() -> Offenses {
    Offenses::of(Rule {
        anchor: "sealed-lint-table",
        fix: "restore the rule and fix the code it flags; a rule that seems wrong goes to the owner, never into config",
    })
}

fn parse(path: &Path) -> Table {
    read(path).parse().unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn lint_entry(value: &Value) -> Option<(Level, i64)> {
    match value {
        Value::String(level) => Some((Level::parse(level)?, 0)),
        Value::Table(entry) => {
            let level = Level::parse(entry.get("level")?.as_str()?)?;
            Some((level, entry.get("priority").map_or(Some(0), Value::as_integer)?))
        }
        _ => None,
    }
}

fn check_group(offenses: &mut Offenses, group: &str, table: &Table, required: &[Required]) {
    for want in required {
        check_required(offenses, group, table.get(want.lint).and_then(lint_entry), want);
    }
    for (lint, value) in table.iter().filter(|(lint, _)| !required.iter().any(|want| want.lint == *lint)) {
        if lint_entry(value).is_none_or(|(level, _)| level == Level::Allow) {
            offenses.add(format!("cli/Cargo.toml: {group}::{lint} = {value} weakens the table"));
        }
    }
}

fn check_required(offenses: &mut Offenses, group: &str, set: Option<(Level, i64)>, want: &Required) {
    let lint = want.lint;
    match set {
        None => offenses.add(format!("cli/Cargo.toml: {group}::{lint} is missing")),
        Some((level, _)) if level < want.level => {
            offenses.add(format!("cli/Cargo.toml: {group}::{lint} is {level:?}, below {:?}", want.level));
        }
        Some((_, priority)) if priority != want.priority => {
            offenses.add(format!("cli/Cargo.toml: {group}::{lint} has priority {priority}, not {}", want.priority));
        }
        Some(_) => {}
    }
}

fn workspace_lints() -> Table {
    let manifest = parse(&cli().join("Cargo.toml"));
    manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("lints"))
        .and_then(Value::as_table)
        .cloned()
        .unwrap_or_default()
}

#[test]
fn the_workspace_lint_table_keeps_every_required_lint_at_its_level() {
    let mut offenses = sealed();
    let lints = workspace_lints();
    for (group, required) in [("rust", RUST), ("clippy", CLIPPY)] {
        let table = lints.get(group).and_then(Value::as_table).cloned().unwrap_or_default();
        check_group(&mut offenses, group, &table, required);
    }
    for group in lints.keys().filter(|group| !["rust", "clippy"].contains(&group.as_str())) {
        offenses.add(format!("cli/Cargo.toml: the [workspace.lints.{group}] table is not part of the gauntlet"));
    }
    offenses.assert_none();
}

#[test]
fn every_crate_of_the_workspace_takes_the_workspace_lints() {
    let mut offenses = sealed();
    let manifest = parse(&cli().join("Cargo.toml"));
    let members = manifest.get("workspace").and_then(|workspace| workspace.get("members")).and_then(Value::as_array);
    let crates = members.into_iter().flatten().filter_map(Value::as_str).map(|member| cli().join(member));
    for dir in std::iter::once(cli().to_path_buf()).chain(crates) {
        let lints = parse(&dir.join("Cargo.toml")).get("lints").cloned();
        if lints != Some(Value::Table(Table::from_iter([("workspace".to_string(), Value::Boolean(true))]))) {
            offenses.add(format!("{}/Cargo.toml: [lints] is not exactly `workspace = true`", relative(&dir, &repo())));
        }
    }
    offenses.assert_none();
}

fn is_known_setting(key: &str) -> bool {
    THRESHOLDS.iter().any(|(threshold, _)| *threshold == key) || IN_TESTS.contains(&key) || OTHER_KEYS.contains(&key)
}

fn check_clippy_config(offenses: &mut Offenses, name: &str, config: &Table, methods: &[&str]) {
    for (key, value) in config.iter().filter(|(key, _)| !is_known_setting(key)) {
        offenses.add(format!("{name}: `{key} = {value}` is not a setting the gauntlet knows"));
    }
    for (threshold, most) in THRESHOLDS {
        if config.get(*threshold).and_then(Value::as_integer).is_none_or(|set| set > *most) {
            offenses.add(format!("{name}: {threshold} must be set to at most {most}"));
        }
    }
    if config.get("avoid-breaking-exported-api").and_then(Value::as_bool) != Some(false) {
        offenses.add(format!("{name}: avoid-breaking-exported-api must be false"));
    }
    check_renamed_params(offenses, name, config);
    check_disallowed_methods(offenses, name, config, methods);
}

fn check_renamed_params(offenses: &mut Offenses, name: &str, config: &Table) {
    let listed = config.get("allow-renamed-params-for").and_then(Value::as_array).cloned().unwrap_or_default();
    for extra in listed.iter().filter(|path| path.as_str().is_none_or(|path| !RENAMED_PARAMS.contains(&path))) {
        offenses.add(format!("{name}: allow-renamed-params-for adds {extra}"));
    }
}

fn check_disallowed_methods(offenses: &mut Offenses, name: &str, config: &Table, methods: &[&str]) {
    let listed = config.get("disallowed-methods").and_then(Value::as_array).cloned().unwrap_or_default();
    let has_reason = |method: &str| {
        listed.iter().any(|entry| {
            entry.get("path").and_then(Value::as_str) == Some(method)
                && entry.get("reason").and_then(Value::as_str).is_some_and(|reason| !reason.is_empty())
        })
    };
    for method in methods.iter().filter(|method| !has_reason(method)) {
        offenses.add(format!("{name}: disallowed-methods lacks {method} with a reason that names the fix"));
    }
}

#[test]
fn clippy_configs_keep_their_thresholds_test_exceptions_and_banned_methods() {
    let mut offenses = sealed();
    let root = repo();
    let configs = files_under(&root, &|path| {
        path.file_name().is_some_and(|file| file == "clippy.toml" || file == ".clippy.toml")
    });
    for path in &configs {
        let name = relative(path, &root);
        match CLIPPY_CONFIGS.iter().find(|(expected, _)| *expected == name) {
            Some((_, methods)) => check_clippy_config(&mut offenses, &name, &parse(path), methods),
            None => {
                offenses.add(format!("{name}: a clippy.toml the gauntlet does not know changes the lints below it"));
            }
        }
    }
    for (expected, _) in CLIPPY_CONFIGS.iter().filter(|(expected, _)| !configs.contains(&root.join(expected))) {
        offenses.add(format!("{expected}: missing"));
    }
    offenses.assert_none();
}

fn offenses_of(group: &str, table: &str, required: &[Required]) -> Vec<String> {
    let mut offenses = sealed();
    check_group(&mut offenses, group, &table.parse().expect("a lint table"), required);
    offenses.into_lines()
}

#[test]
fn a_missing_lowered_reprioritised_or_extra_allowed_lint_is_an_offense() {
    let required = [
        Required { lint: "pedantic", level: Level::Warn, priority: -1 },
        Required { lint: "unwrap_used", level: Level::Deny, priority: 0 },
    ];
    let found = offenses_of(
        "clippy",
        "pedantic = \"warn\"\nunwrap_used = \"warn\"\nmodule_name_repetitions = \"allow\"\n",
        &required,
    );
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(found[0].starts_with("cli/Cargo.toml: clippy::pedantic has priority 0, not -1"), "{found:?}");
    assert!(found[1].starts_with("cli/Cargo.toml: clippy::unwrap_used is Warn, below Deny"), "{found:?}");
    assert!(found[2].starts_with("cli/Cargo.toml: clippy::module_name_repetitions = \"allow\" weakens"), "{found:?}");
    let stronger = "pedantic = { level = \"deny\", priority = -1 }\nunwrap_used = \"forbid\"\nexit = \"deny\"\n";
    assert_eq!(offenses_of("clippy", stronger, &required), Vec::<String>::new());
}

#[test]
fn a_raised_threshold_an_extra_exception_or_a_dropped_ban_is_an_offense() {
    let mut offenses = sealed();
    let config: Table = "cognitive-complexity-threshold = 7\ntoo-many-arguments-threshold = 6\ntoo-many-lines-threshold = 40\nexcessive-nesting-threshold = 4\navoid-breaking-exported-api = false\nallow-dbg-in-tests = true\ndisallowed-methods = [{ path = \"std::process::exit\", reason = \"return an ExitCode\" }]\n"
        .parse()
        .expect("a clippy.toml");
    check_clippy_config(&mut offenses, "cli/clippy.toml", &config, EXIT_AND_ABORT);
    let found = offenses.into_lines();
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(found[0].starts_with("cli/clippy.toml: `allow-dbg-in-tests = true` is not a setting"), "{found:?}");
    assert!(
        found[1].starts_with("cli/clippy.toml: cognitive-complexity-threshold must be set to at most 6"),
        "{found:?}"
    );
    assert!(found[2].starts_with("cli/clippy.toml: disallowed-methods lacks std::process::abort"), "{found:?}");
}
