use anyhow::{Context, Result};

use crate::offenses::{Rule, Verdict};
use crate::regions::{Region, Regions};
use crate::rust::{Function, functions};
use crate::workspace::Workspace;

const CEILING: f64 = 6.0;

const RULE: Rule = Rule {
    anchor: "crap",
    fix: "cover every branch through the crate's real surface, or split it into simpler functions that are each covered",
};

pub(crate) struct Score {
    pub(crate) crap: f64,
    pub(crate) covered: f64,
    pub(crate) regions: usize,
    pub(crate) ran: usize,
}

pub(crate) fn score(function: &Function, regions: &[Region]) -> Score {
    let owned: Vec<&Region> = regions.iter().filter(|region| function.owns(region.line)).collect();
    let ran = owned.iter().filter(|region| region.ran).count();
    let covered = match (u32::try_from(ran), u32::try_from(owned.len())) {
        (Ok(covered), Ok(total)) if total > 0 => f64::from(covered) / f64::from(total),
        _ => 0.0,
    };
    let complexity = f64::from(function.cyclomatic);
    Score { crap: complexity * complexity * (1.0 - covered).powi(3) + complexity, covered, regions: owned.len(), ran }
}

fn offense(function: &Function, score: &Score) -> Option<String> {
    if score.regions > 0 && score.ran == 0 {
        return Some(format!("{} ran under no test: none of its {} regions ran", function.name, score.regions));
    }
    (score.crap > CEILING).then(|| {
        format!(
            "{} has CRAP {:.1} (cyclomatic {}, {:.0}% of its regions covered), over {CEILING}",
            function.name,
            score.crap,
            function.cyclomatic,
            score.covered * 100.0
        )
    })
}

pub(crate) fn check(workspace: &Workspace, regions: &Regions) -> Result<Verdict> {
    let mut offenses = Vec::new();
    let mut scored = 0usize;
    for file in workspace.members.iter().flat_map(|member| &member.sources) {
        let source = std::fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
        let found = functions(&source).with_context(|| format!("parse {}", file.display()))?;
        for function in &found {
            if let Some(offense) = offense(function, &score(function, regions.of(file))) {
                offenses.push(format!("{}:{}: {offense}", workspace.shown(file), function.lines.start()));
            }
        }
        scored += found.len();
    }
    Ok(RULE.verdict(&offenses, vec![format!("{scored} functions at CRAP {CEILING} or under")]))
}
