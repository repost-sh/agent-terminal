use std::fmt;

pub(crate) struct Rule {
    pub(crate) anchor: &'static str,
    pub(crate) fix: &'static str,
}

pub(crate) struct Verdict {
    pub(crate) lines: Vec<String>,
    pub(crate) passed: bool,
}

impl Verdict {
    pub(crate) fn of(rule: &Rule, offenses: &[String], passes: Vec<String>) -> Self {
        let broken: Vec<(String, &Rule)> = offenses.iter().map(|offense| (offense.clone(), rule)).collect();
        Self::of_rules(&broken, passes)
    }

    pub(crate) fn of_rules(offenses: &[(String, &Rule)], passes: Vec<String>) -> Self {
        if offenses.is_empty() {
            return Self { lines: passes, passed: true };
        }
        let lines = offenses
            .iter()
            .map(|(offense, rule)| line(offense, rule))
            .chain([format!("{} offense(s)", offenses.len())])
            .collect();
        Self { lines, passed: false }
    }
}

fn line(offense: impl fmt::Display, rule: &Rule) -> String {
    format!("{offense}: {} → GAUNTLET.md#{}", rule.fix, rule.anchor)
}
