use crate::offenses::{Offenses, Rule};
use crate::repo_files::{cli, files_under, read, relative, repo};

fn scratch_folders(gitignore: &str) -> Vec<&str> {
    gitignore
        .lines()
        .map(str::trim)
        .filter(|entry| entry.starts_with('.') && entry.ends_with('/'))
        .map(|entry| entry.trim_end_matches('/'))
        .collect()
}

#[test]
fn no_source_names_a_gitignored_scratch_folder() {
    let mut offenses = Offenses::of(Rule {
        anchor: "artifact-paths",
        fix: "tests and code never read gitignored scratch; use a temporary folder the test creates (support::scratch)",
    });
    let root = repo();
    let gitignore = read(&root.join(".gitignore"));
    let folders = scratch_folders(&gitignore);
    let scanned = [cli().to_path_buf(), root.join("dashboard/src"), root.join("npm"), root.join("scripts")];
    for file in scanned.iter().flat_map(|dir| files_under(dir, &|_| true)) {
        let bytes = std::fs::read(&file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        for folder in
            folders.iter().filter(|folder| bytes.windows(folder.len()).any(|window| window == folder.as_bytes()))
        {
            offenses.add(format!("{}: names {folder}", relative(&file, &root)));
        }
    }
    offenses.assert_none();
}

#[test]
fn the_scratch_folders_are_the_ignored_dot_folders() {
    assert_eq!(scratch_folders("target/\n.scratch/\n*.snap.new\n.tool/cache/\n"), [".scratch", ".tool/cache"]);
}
