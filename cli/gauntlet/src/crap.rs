use anyhow::{Context, Result};

use crate::lcov::{Hits, Lines};
use crate::offenses::{Rule, Verdict};
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
}

pub(crate) fn score(function: &Function, hits: Option<&Hits>) -> Score {
    let owned: Vec<u64> =
        hits.into_iter().flatten().filter(|&(&line, _)| function.owns(line)).map(|(_, &hit)| hit).collect();
    let covered = owned.iter().filter(|&&hit| hit > 0).count();
    let covered = match (u32::try_from(covered), u32::try_from(owned.len())) {
        (Ok(covered), Ok(total)) if total > 0 => f64::from(covered) / f64::from(total),
        _ => 0.0,
    };
    let complexity = f64::from(function.cyclomatic);
    Score { crap: complexity * complexity * (1.0 - covered).powi(3) + complexity, covered }
}

pub(crate) fn check(workspace: &Workspace, lines: &Lines) -> Result<Verdict> {
    let mut offenses = Vec::new();
    let mut scored = 0usize;
    for file in workspace.members.iter().flat_map(|member| &member.sources) {
        let source = std::fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
        let found = functions(&source).with_context(|| format!("parse {}", file.display()))?;
        for function in &found {
            let score = score(function, lines.of(file));
            if score.crap > CEILING {
                offenses.push(format!(
                    "{}:{}: {} has CRAP {:.1} (cyclomatic {}, {:.0}% of its lines covered), over {CEILING}",
                    workspace.shown(file),
                    function.lines.start(),
                    function.name,
                    score.crap,
                    function.cyclomatic,
                    score.covered * 100.0
                ));
            }
        }
        scored += found.len();
    }
    Ok(Verdict::of(&RULE, &offenses, vec![format!("{scored} functions at CRAP {CEILING} or under")]))
}
