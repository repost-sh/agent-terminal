pub(crate) mod common;
pub(crate) mod input;
pub(crate) mod screen;
pub(crate) mod sessions;

use gherkin_cargo_test::{Ctx, DataTable, StepRegistry};

use crate::terminal::Terminal;

pub(crate) type Step = fn(&mut Ctx<Terminal>, &[String], Option<&DataTable>);

pub(crate) struct Steps<'a>(pub(crate) &'a mut StepRegistry<Terminal>);

impl Steps<'_> {
    pub(crate) fn given(&mut self, pattern: &str, step: Step) -> &mut Self {
        self.0.define(pattern, step);
        self
    }

    pub(crate) fn when(&mut self, pattern: &str, step: Step) -> &mut Self {
        self.0.define(pattern, step);
        self
    }

    pub(crate) fn then(&mut self, pattern: &str, step: Step) -> &mut Self {
        self.0.define(pattern, step);
        self
    }
}

pub(crate) fn arg(args: &[String], at: usize) -> &str {
    args.get(at).map_or("", String::as_str)
}
