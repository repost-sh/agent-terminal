use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::offenses::{Rule, Verdict};
use crate::rust::{Bags, bags};
use crate::workspace::{Member, Workspace};

const RULE: Rule = Rule {
    anchor: "parameter-bags",
    fix: "pass what the function needs itself, or give the struct the methods that use its fields",
};

struct Seen {
    shapes: Vec<(PathBuf, String, usize)>,
    built: BTreeMap<String, usize>,
    taken: BTreeMap<String, usize>,
}

fn seen(member: &Member) -> Result<Seen> {
    let mut seen = Seen { shapes: Vec::new(), built: BTreeMap::new(), taken: BTreeMap::new() };
    for file in member.sources.iter().chain(&member.unit_tests).chain(&member.tests) {
        let source = std::fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
        let Bags { shapes, built, taken } = bags(&source).with_context(|| format!("parse {}", file.display()))?;
        if member.sources.contains(file) {
            seen.shapes.extend(shapes.into_iter().map(|shape| (file.clone(), shape.name, shape.line)));
        }
        for name in built {
            *seen.built.entry(name).or_default() += 1;
        }
        for name in taken {
            *seen.taken.entry(name).or_default() += 1;
        }
    }
    Ok(seen)
}

pub(crate) fn check(workspace: &Workspace) -> Result<Verdict> {
    let mut offenses = Vec::new();
    let mut scanned = 0;
    for member in &workspace.members {
        let Seen { shapes, built, taken } = seen(member)?;
        scanned += shapes.len();
        let once = |counts: &BTreeMap<String, usize>, name: &String| counts.get(name) == Some(&1);
        offenses.extend(shapes.iter().filter(|(_, name, _)| once(&built, name) && once(&taken, name)).map(
            |(file, name, line)| {
                format!("{}:{line}: {name} is built in one place and taken by one function", workspace.shown(file))
            },
        ));
    }
    Ok(RULE.verdict(&offenses, vec![format!("{scanned} structs with named fields, none built once for one function")]))
}
