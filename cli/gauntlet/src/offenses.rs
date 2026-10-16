use std::fmt;

pub(crate) struct Rule {
    pub(crate) anchor: &'static str,
    pub(crate) fix: &'static str,
}

pub(crate) struct Verdict {
    pub(crate) lines: Vec<String>,
    pub(crate) passed: bool,
}

impl Rule {
    pub(crate) fn verdict(&self, offenses: &[String], passes: Vec<String>) -> Verdict {
        let broken: Vec<(String, &Rule)> = offenses.iter().map(|offense| (offense.clone(), self)).collect();
        Verdict::of_rules(&broken, passes)
    }
}

impl Verdict {
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
