use std::fmt;

pub(crate) struct Rule {
    pub(crate) anchor: &'static str,
    pub(crate) fix: &'static str,
}

pub(crate) struct Offenses {
    rule: Rule,
    found: Vec<String>,
}

impl Offenses {
    pub(crate) fn of(rule: Rule) -> Self {
        Self { rule, found: Vec::new() }
    }

    pub(crate) fn add(&mut self, offense: impl fmt::Display) {
        self.found.push(format!("{offense}: {} → GAUNTLET.md#{}", self.rule.fix, self.rule.anchor));
    }

    pub(crate) fn into_lines(self) -> Vec<String> {
        self.found
    }

    #[track_caller]
    pub(crate) fn assert_none(&self) {
        assert!(self.found.is_empty(), "{} offense(s):\n{}", self.found.len(), self.found.join("\n"));
    }
}

#[test]
fn an_offense_line_ends_with_its_fix_and_its_rule() {
    let mut offenses = Offenses::of(Rule { anchor: "no-comments", fix: "say it with a name, a type or a test" });
    offenses.add("src/main.rs:3: a comment");
    assert_eq!(
        offenses.found,
        ["src/main.rs:3: a comment: say it with a name, a type or a test → GAUNTLET.md#no-comments"]
    );
}
