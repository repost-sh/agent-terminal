use std::path::{Path, PathBuf};

use crate::offenses::{Offenses, Rule};
use crate::repo_files::{cli, files_under, read, relative, repo, rust_files};
use crate::rust_source::scan;

const LINT_FLAGS: &[&str] =
    &["RUSTFLAGS", "cap-lints", "CLIPPY_CONF_DIR", "-A clippy", "-Aclippy", "-A warnings", "-Awarnings", "--no-verify"];

const TEST_ROOTS: &[&str] = &["tests/cli/main.rs", "tests/static_checks/main.rs", "perf/tests/perf/main.rs"];

fn sealed() -> Offenses {
    Offenses::of(Rule {
        anchor: "sealed-toolchain",
        fix: "lints are set only in cli/Cargo.toml and the clippy.toml files; remove the flag and fix the code it hid",
    })
}

fn crate_dirs() -> Vec<PathBuf> {
    vec![cli().to_path_buf(), cli().join("perf"), cli().join("tests/fixtures")]
}

fn flags_in(text: &str) -> Vec<&'static str> {
    LINT_FLAGS.iter().copied().filter(|flag| text.contains(flag)).collect()
}

#[test]
fn no_cargo_config_build_script_hook_or_workflow_passes_lint_flags() {
    let mut offenses = sealed();
    let root = repo();
    let configs = std::iter::once(root.clone())
        .chain(crate_dirs())
        .flat_map(|dir| ["config", "config.toml"].map(|name| dir.join(".cargo").join(name)));
    let builds = crate_dirs().into_iter().map(|dir| dir.join("build.rs"));
    let automation = [".githooks", "scripts", ".github/workflows"]
        .into_iter()
        .map(|dir| root.join(dir))
        .filter(|dir| dir.is_dir())
        .flat_map(|dir| files_under(&dir, &|_| true));
    for file in configs.chain(builds).chain(automation).filter(|file| file.is_file()) {
        for flag in flags_in(&read(&file)) {
            offenses.add(format!("{}: passes `{flag}`", relative(&file, &root)));
        }
    }
    offenses.assert_none();
}

fn test_attributes(code: &str) -> Vec<(usize, String)> {
    let attribute = regex::Regex::new(r"#!?\[\s*cfg(?:_attr)?\s*\(([^\]]*)\]").expect("the cfg pattern");
    attribute
        .captures_iter(code)
        .filter(|found| found[1].split(|c: char| !c.is_alphanumeric() && c != '_').any(|word| word == "test"))
        .map(|found| {
            let whole = found.get(0).map_or("", |whole| whole.as_str());
            let line = code[..code.find(whole).unwrap_or(0)].matches('\n').count() + 1;
            (line, whole.to_string())
        })
        .collect()
}

fn marks_a_module_declaration(code: &str, line: usize, attribute: &str) -> bool {
    let next = code.lines().skip(line).map(str::trim).find(|text| !text.is_empty()).unwrap_or_default();
    attribute == "#[cfg(test)]" && next.starts_with("mod ") && next.ends_with(';')
}

fn cfg_test_offenses(relative: &str, code: &str) -> Vec<String> {
    let in_a_root = TEST_ROOTS.contains(&relative);
    test_attributes(code)
        .into_iter()
        .filter(|(line, attribute)| !(in_a_root && marks_a_module_declaration(code, *line, attribute)))
        .map(|(line, attribute)| format!("{relative}:{line}: `{attribute}` makes code count as a test"))
        .collect()
}

#[test]
fn cfg_test_only_marks_the_modules_of_the_test_crate_roots() {
    let mut offenses = sealed();
    for file in rust_files() {
        let relative = relative(&file, cli());
        for offense in cfg_test_offenses(&relative, &scan(&read(&file)).code) {
            offenses.add(offense);
        }
    }
    offenses.assert_none();
}

#[test]
fn cfg_test_outside_a_root_or_on_anything_but_a_module_is_an_offense() {
    assert_eq!(cfg_test_offenses("tests/cli/main.rs", "#[cfg(test)]\nmod support;\n"), Vec::<String>::new());
    assert_eq!(
        cfg_test_offenses("tests/cli/main.rs", "#[cfg(test)]\nfn helper() {}\n"),
        ["tests/cli/main.rs:1: `#[cfg(test)]` makes code count as a test"]
    );
    assert_eq!(
        cfg_test_offenses("src/output.rs", "fn a() {}\n#[cfg(any(test, feature = \"x\"))]\nmod tests;\n"),
        ["src/output.rs:2: `#[cfg(any(test, feature = \"x\"))]` makes code count as a test"]
    );
}

#[test]
fn a_flag_that_lowers_lints_is_found_in_text() {
    assert_eq!(flags_in("cargo clippy -- -A clippy::unwrap_used"), ["-A clippy"]);
    assert_eq!(flags_in("RUSTFLAGS=\"--cap-lints warn\" cargo build"), ["RUSTFLAGS", "cap-lints"]);
    assert_eq!(flags_in("git commit --no-verify"), ["--no-verify"]);
    assert_eq!(flags_in("cargo clippy --all-targets -- -D warnings"), Vec::<&str>::new());
}

#[test]
fn the_crates_checked_are_the_workspace_members() {
    let dirs: Vec<PathBuf> = crate_dirs();
    assert!(dirs.iter().all(|dir| dir.join("Cargo.toml").is_file()), "{dirs:?}");
    assert_eq!(dirs.first().map(PathBuf::as_path), Some(Path::new(env!("CARGO_MANIFEST_DIR"))));
}
