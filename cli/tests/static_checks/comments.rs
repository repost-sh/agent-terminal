use crate::offenses::{Offenses, Rule};
use crate::repo_files::{cli, read, relative, repo, rust_files};
use crate::rust_source::scan;

#[test]
fn rust_files_have_no_comments() {
    let mut offenses = Offenses::of(Rule {
        anchor: "no-comments",
        fix: "say it with a name, a type or a test; a reason belongs in the commit message",
    });
    let files = rust_files();
    assert!(!files.is_empty(), "no Rust files found under {}", cli().display());
    for file in &files {
        for line in scan(&read(file)).comment_lines {
            offenses.add(format!("{}:{line}: a comment", relative(file, &repo())));
        }
    }
    offenses.assert_none();
}
