use anyhow::{Context, Result};

use crate::offenses::{Rule, Verdict};
use crate::rust::has_property_test;
use crate::workspace::{Member, Workspace};

const RULE: Rule = Rule {
    anchor: "property-tests",
    fix: "state an invariant of the crate's behaviour as a proptest! with a #[test], an input strategy and an assertion",
};

pub(crate) fn check(workspace: &Workspace) -> Result<Verdict> {
    let mut offenses = Vec::new();
    let mut passes = Vec::new();
    for member in &workspace.members {
        if has_one(member)? {
            passes.push(format!("{}: has a property test", member.name));
        } else {
            offenses.push(format!("{}: no property test", member.name));
        }
    }
    Ok(Verdict::of(&RULE, &offenses, passes))
}

fn has_one(member: &Member) -> Result<bool> {
    for file in member.sources.iter().chain(&member.tests) {
        let source = std::fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
        if has_property_test(&source).with_context(|| format!("parse {}", file.display()))? {
            return Ok(true);
        }
    }
    Ok(false)
}
