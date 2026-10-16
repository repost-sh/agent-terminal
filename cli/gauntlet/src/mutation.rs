use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::offenses::{Rule, Verdict};

const RULE: Rule = Rule {
    anchor: "mutation",
    fix: "write a test whose expected value changes when the mutated line does, through the crate's real surface",
};

const TIMEOUTS_PERCENT: usize = 5;

#[derive(Deserialize)]
struct Outcomes {
    total_mutants: usize,
    outcomes: Vec<Outcome>,
}

#[derive(Deserialize)]
struct Outcome {
    scenario: Scenario,
    summary: String,
}

#[derive(Deserialize)]
enum Scenario {
    Baseline,
    Mutant(Mutant),
}

#[derive(Deserialize)]
struct Mutant {
    name: String,
}

#[derive(Default)]
struct Tally {
    total: usize,
    caught: usize,
    unviable: usize,
    timeouts: usize,
    offenses: Vec<String>,
}

pub(crate) fn check(reports: &[&Path]) -> Result<Verdict> {
    let mut tally = Tally::default();
    for report in reports {
        let text = std::fs::read_to_string(report).with_context(|| format!("read {}", report.display()))?;
        let read: Outcomes = serde_json::from_str(&text).with_context(|| format!("parse {}", report.display()))?;
        tally.add(&report.display().to_string(), &read);
    }
    tally.guard();
    let passed = format!(
        "{} mutants: {} caught, {} unviable, {} timed out, none missed",
        tally.total, tally.caught, tally.unviable, tally.timeouts
    );
    Ok(Verdict::of(&RULE, &tally.offenses, vec![passed]))
}

impl Tally {
    fn add(&mut self, report: &str, read: &Outcomes) {
        let baseline = read.outcomes.iter().find(|outcome| matches!(outcome.scenario, Scenario::Baseline));
        if baseline.is_none_or(|baseline| baseline.summary != "Success") {
            self.offenses
                .push(format!("{report}: the unmutated baseline did not pass, so no mutant there proves anything"));
        }
        let mutants: Vec<(&Mutant, &str)> = read
            .outcomes
            .iter()
            .filter_map(|outcome| match &outcome.scenario {
                Scenario::Mutant(mutant) => Some((mutant, outcome.summary.as_str())),
                Scenario::Baseline => None,
            })
            .collect();
        if mutants.len() != read.total_mutants {
            self.offenses.push(format!(
                "{report}: {} of its {} mutants were tested",
                mutants.len(),
                read.total_mutants
            ));
        }
        for (mutant, summary) in mutants {
            self.count(mutant, summary);
        }
    }

    fn count(&mut self, mutant: &Mutant, summary: &str) {
        self.total += 1;
        match summary {
            "CaughtMutant" => self.caught += 1,
            "Unviable" => self.unviable += 1,
            "Timeout" => self.timeouts += 1,
            _ => self.offenses.push(format!("{} survived ({summary})", mutant.name)),
        }
    }

    fn guard(&mut self) {
        if self.caught == 0 {
            self.offenses.push("no mutant was caught, so the tests ran on nothing".to_string());
        }
        if self.timeouts * 100 > self.total * TIMEOUTS_PERCENT {
            self.offenses.push(format!(
                "{} of {} mutants timed out, over {TIMEOUTS_PERCENT}%: a timeout hides whether the tests catch it; run with less load",
                self.timeouts, self.total
            ));
        }
    }
}
