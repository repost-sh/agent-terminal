use std::path::Path;

use crate::offenses::{Offenses, Rule};
use crate::repo_files::{cli, files_under, read, relative, repo};

const MAX_LINES: usize = 500;

fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| extensions.contains(&extension))
}

#[test]
fn no_source_file_is_over_500_lines() {
    let mut offenses = Offenses::of(Rule {
        anchor: "file-size",
        fix: "split the file by concern into a folder with one file per concern (AGENTS.md, No Junk Drawers)",
    });
    let rust = files_under(cli(), &|path| has_extension(path, &["rs"]));
    let dashboard = files_under(&repo().join("dashboard"), &|path| has_extension(path, &["ts", "tsx", "js"]));
    for file in rust.iter().chain(&dashboard) {
        let lines = read(file).lines().count();
        if lines > MAX_LINES {
            offenses.add(format!("{}: {lines} lines, over {MAX_LINES}", relative(file, &repo())));
        }
    }
    offenses.assert_none();
}
