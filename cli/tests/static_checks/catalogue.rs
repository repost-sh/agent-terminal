use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::offenses::{Offenses, Rule};
use crate::repo_files::{cli, files_under, read, relative, repo};

const MARK: &str = concat!("GAUNTLET.md", "#");

const ANCHOR: &str = concat!("anchor: ", "\"");

fn slug(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
        .collect::<String>()
        .replace(' ', "-")
}

fn sections(catalogue: &str) -> Vec<String> {
    catalogue
        .lines()
        .filter_map(|line| line.strip_prefix("## ").or_else(|| line.strip_prefix("### ")))
        .map(slug)
        .collect()
}

fn named(text: &str) -> Vec<String> {
    let after = |marker: &'static str| {
        text.match_indices(marker).map(move |(at, _)| {
            text.get(at + marker.len()..)
                .unwrap_or_default()
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
                .collect::<String>()
        })
    };
    after(MARK).chain(after(ANCHOR)).filter(|anchor| !anchor.is_empty()).collect()
}

fn with_extension(extensions: &'static [&'static str]) -> impl Fn(&Path) -> bool {
    move |path| path.extension().and_then(|extension| extension.to_str()).is_some_and(|ext| extensions.contains(&ext))
}

fn checked_files() -> Vec<PathBuf> {
    let root = repo();
    let mut files = files_under(cli(), &with_extension(&["rs"]));
    files.extend(files_under(&root.join("dashboard"), &with_extension(&["ts", "tsx", "js"])));
    files.extend(files_under(&root.join("scripts"), &|_| true));
    files.extend(files_under(&root.join(".githooks"), &|_| true));
    files
}

#[test]
fn every_rule_an_offense_names_has_a_section_in_gauntlet_md() {
    let mut offenses = Offenses::of(Rule {
        anchor: "catalogue",
        fix: "add the rule's section to GAUNTLET.md, or point the check at the section that explains it",
    });
    let root = repo();
    let known = sections(&read(&root.join("GAUNTLET.md")));
    let files = checked_files();
    assert!(files.iter().any(|file| file.ends_with("scripts/gauntlet.sh")), "{files:?}");
    for file in &files {
        for anchor in named(&read(file)).into_iter().collect::<BTreeSet<_>>() {
            if !known.contains(&anchor) {
                offenses.add(format!("{}: {MARK}{anchor} names no section of GAUNTLET.md", relative(file, &root)));
            }
        }
    }
    offenses.assert_none();
}

#[test]
fn headings_become_anchors_and_offense_text_names_them() {
    assert_eq!(
        sections("# The gauntlet\n## Rule Zero\n### Sealed ESLint config\n#### deeper\n"),
        ["rule-zero", "sealed-eslint-config"]
    );
    let text = format!("x → {MARK}crap\n{ANCHOR}no-comments\", fix\n{MARK}{{}}\n");
    assert_eq!(named(&text), ["crap", "no-comments"]);
}
