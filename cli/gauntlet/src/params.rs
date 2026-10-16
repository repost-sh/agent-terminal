use anyhow::{Context, Result};

use crate::offenses::{Rule, Verdict};
use crate::rust::shared_functions;
use crate::workspace::Workspace;

const CEILING: usize = 2;

const RULE: Rule = Rule {
    anchor: "parameter-ceiling",
    fix: "group the parameters into one struct with named fields, or keep the function inside its module",
};

pub(crate) fn check(workspace: &Workspace) -> Result<Verdict> {
    let mut offenses = Vec::new();
    let mut shared = 0;
    for member in &workspace.members {
        for file in member.sources.iter().chain(&member.tests) {
            let source = std::fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
            let functions = shared_functions(&source).with_context(|| format!("parse {}", file.display()))?;
            shared += functions.len();
            offenses.extend(functions.iter().filter(|function| function.count > CEILING).map(|function| {
                format!(
                    "{}:{}: {} takes {} parameters",
                    workspace.shown(file),
                    function.line,
                    function.function,
                    function.count
                )
            }));
        }
    }
    let passes = vec![format!("{shared} functions shared beyond their module take at most {CEILING} parameters")];
    Ok(RULE.verdict(&offenses, passes))
}
