use super::policy::Role;
use crate::offenses::Rule;
use crate::workspace::{Kind, Workspace};

const DEPENDS_ACROSS: Rule = Rule {
    anchor: "architecture",
    fix: "drive agent-terminal through its built binary, or move the shared code into the crate that needs it",
};

const UNCLASSIFIED: Rule =
    Rule { anchor: "architecture", fix: "give the crate a role in gauntlet/src/architecture/policy.rs" };

pub(super) fn offenses(workspace: &Workspace) -> Vec<(String, &'static Rule)> {
    let mut found = Vec::new();
    for member in &workspace.members {
        let Some(role) = Role::of(&member.name) else {
            found.push((format!("{}: the crate has no role", member.name), &UNCLASSIFIED));
            continue;
        };
        for dependency in &member.dependencies {
            let kind = dependency.kind.unwrap_or(Kind::Normal);
            let Some(other) = Role::of(&dependency.name) else {
                continue;
            };
            if !role.may_depend(kind, other) {
                let offense = format!("{} ({role}): a {kind} dependency on {} ({other})", member.name, dependency.name);
                found.push((offense, &DEPENDS_ACROSS));
            }
        }
    }
    found
}
