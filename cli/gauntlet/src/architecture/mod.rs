mod crates;
mod modules;
mod policy;

use anyhow::Result;

use crate::offenses::{Rule, Verdict};
use crate::workspace::Workspace;

const NO_PRODUCT: Rule = Rule {
    anchor: "architecture",
    fix: "run the gate in the workspace whose agent-terminal crate the policy describes",
};

pub(crate) fn check(workspace: &Workspace) -> Result<Verdict> {
    let mut offenses = crates::offenses(workspace);
    if !workspace.members.iter().any(|member| member.name == policy::PRODUCT) {
        offenses.push((format!("the workspace has no {} crate", policy::PRODUCT), &NO_PRODUCT));
    }
    let mut passes = vec!["every crate depends on another only as its role allows".to_string()];
    for member in workspace.members.iter().filter(|member| member.name == policy::PRODUCT) {
        offenses.extend(modules::offenses(workspace, member)?);
        passes.push(format!("{}: every module keeps to its layer", member.name));
    }
    Ok(Verdict::of_rules(&offenses, passes))
}
