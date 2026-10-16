use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::offenses::{Rule, Verdict};
use crate::rust::{Helper, names_used, one_expression_helpers};
use crate::workspace::{Member, Workspace};

const RULE: Rule = Rule {
    anchor: "trivial-helpers",
    fix: "write the expression at its one call site, or keep the helper for a second caller that needs it",
};

fn read(file: &Path) -> Result<String> {
    std::fs::read_to_string(file).with_context(|| format!("read {}", file.display()))
}

fn uses(member: &Member) -> Result<BTreeMap<String, usize>> {
    let mut used = BTreeMap::new();
    for file in member.sources.iter().chain(&member.unit_tests).chain(&member.tests) {
        for name in names_used(&read(file)?).with_context(|| format!("parse {}", file.display()))? {
            *used.entry(name).or_default() += 1;
        }
    }
    Ok(used)
}

fn helpers(member: &Member) -> Result<Vec<(&PathBuf, Helper)>> {
    let mut found = Vec::new();
    for file in &member.sources {
        let helpers = one_expression_helpers(&read(file)?).with_context(|| format!("parse {}", file.display()))?;
        found.extend(helpers.into_iter().map(|helper| (file, helper)));
    }
    Ok(found)
}

pub(crate) fn check(workspace: &Workspace) -> Result<Verdict> {
    let mut offenses = Vec::new();
    let mut scanned = 0;
    for member in &workspace.members {
        let used = uses(member)?;
        let found = helpers(member)?;
        scanned += found.len();
        offenses.extend(found.iter().filter(|(_, helper)| used.get(&helper.name) == Some(&1)).map(|(file, helper)| {
            format!(
                "{}:{}: {} has one call site and its body is one expression",
                workspace.shown(file),
                helper.line,
                helper.name
            )
        }));
    }
    Ok(RULE.verdict(&offenses, vec![format!("{scanned} one-expression private functions, each called more than once")]))
}
