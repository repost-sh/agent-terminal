use std::fmt::Write;
use std::path::Path;

use anyhow::{Context, Result};

use crate::lcov::Lines;
use crate::offenses::{Rule, Verdict};
use crate::rust::functions;
use crate::workspace::{Member, Workspace};

const FLOOR: u32 = 90;

const RULE: Rule = Rule {
    anchor: "coverage-floor",
    fix: "test the uncovered behaviour through the crate's real surface, or delete the branch no surface reaches",
};

#[derive(Default)]
struct Tally {
    covered: u32,
    total: u32,
    missing: Vec<String>,
    least: Vec<(u32, String)>,
}

pub(crate) fn check(workspace: &Workspace, lines: &Lines) -> Result<Verdict> {
    let mut offenses = Vec::new();
    let mut passes = Vec::new();
    for member in &workspace.members {
        let tally = tally(workspace, member, lines)?;
        offenses.extend(tally.missing.iter().map(|file| format!("{file}: compiled into no measured binary")));
        let shown = format!("{}: {} of {} lines covered", member.name, tally.covered, tally.total);
        if tally.total == 0 {
            offenses.push(format!("{}: no line of it was measured", member.name));
        } else if tally.covered * 100 < tally.total * FLOOR {
            offenses.push(format!("{shown}, under the {FLOOR}% floor; most uncovered: {}", most_uncovered(tally)?));
        } else {
            passes.push(shown);
        }
    }
    Ok(RULE.verdict(&offenses, passes))
}

fn tally(workspace: &Workspace, member: &Member, lines: &Lines) -> Result<Tally> {
    let mut tally = Tally::default();
    for file in &member.sources {
        let Some(hits) = lines.of(file) else {
            if has_functions(file)? {
                tally.missing.push(workspace.shown(file));
            }
            continue;
        };
        let total = u32::try_from(hits.len()).context("count the lines")?;
        let covered = u32::try_from(hits.values().filter(|&&hit| hit > 0).count()).context("count the lines")?;
        tally.covered += covered;
        tally.total += total;
        tally.least.push((total - covered, workspace.shown(file)));
    }
    Ok(tally)
}

fn has_functions(file: &Path) -> Result<bool> {
    let source = std::fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
    Ok(!functions(&source).with_context(|| format!("parse {}", file.display()))?.is_empty())
}

fn most_uncovered(mut tally: Tally) -> Result<String> {
    tally.least.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let mut shown = String::new();
    for (uncovered, file) in tally.least.iter().filter(|(uncovered, _)| *uncovered > 0).take(5) {
        let separator = if shown.is_empty() { "" } else { ", " };
        write!(shown, "{separator}{file} ({uncovered})")?;
    }
    Ok(shown)
}
