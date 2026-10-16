use std::path::Path;

use anyhow::{Context, Result};
use gherkin_cargo_test::{ParsedFeature, Step, parse_feature};
use regex::Regex;
use serde_json::{Value, json};

use crate::offenses::{Rule, Verdict};
use crate::rust::StepCode;
use crate::workspace::{Member, Workspace};

const RULE: Rule = Rule {
    anchor: "acceptance",
    fix: "bind each Then step through then(…) to a step that asserts, keep features/ir.json what `at-gauntlet gherkin-ir` writes, and mark nothing wip",
};

struct Feature {
    path: String,
    parsed: ParsedFeature,
}

pub(crate) fn check(workspace: &Workspace) -> Result<Verdict> {
    let suites: Vec<&Member> =
        workspace.members.iter().filter(|member| member.root.join("features").is_dir()).collect();
    let mut offenses = Vec::new();
    if suites.is_empty() {
        offenses.push("the workspace has no crate with acceptance features".to_string());
    }
    let mut passes = Vec::new();
    for member in suites {
        let found = suite_offenses(workspace, member)?;
        if found.is_empty() {
            passes.push(format!("{}: every Then step asserts and features/ir.json is current", member.name));
        }
        offenses.extend(found);
    }
    Ok(Verdict::of(&RULE, &offenses, passes))
}

pub(crate) fn write_ir(workspace: &Workspace) -> Result<Verdict> {
    let mut written = Vec::new();
    for member in workspace.members.iter().filter(|member| member.root.join("features").is_dir()) {
        let path = member.root.join("features").join("ir.json");
        std::fs::write(&path, ir(&features(&member.root.join("features"))?)?)
            .with_context(|| format!("write {}", path.display()))?;
        written.push(format!("wrote {}", workspace.shown(&path)));
    }
    Ok(Verdict { lines: written, passed: true })
}

fn features(dir: &Path) -> Result<Vec<Feature>> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("read {}", dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()?;
    paths.retain(|path| path.extension().is_some_and(|extension| extension == "feature"));
    paths.sort();
    paths
        .iter()
        .map(|path| {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
            let parsed = parse_feature(&text, &name).with_context(|| format!("parse {}", path.display()))?;
            Ok(Feature { path: name, parsed })
        })
        .collect()
}

fn step_json(step: &Step) -> Value {
    json!({ "keyword": step.keyword, "text": step.text, "line": step.line, "table": step.table })
}

fn ir(features: &[Feature]) -> Result<String> {
    let features: Vec<Value> = features
        .iter()
        .map(|feature| {
            let scenarios: Vec<Value> = feature
                .parsed
                .scenarios
                .iter()
                .map(|scenario| {
                    let steps: Vec<Value> = scenario.steps.iter().map(step_json).collect();
                    json!({ "name": scenario.name, "line": scenario.line, "tags": scenario.tags, "steps": steps })
                })
                .collect();
            let background: Vec<Value> = feature.parsed.background.iter().map(step_json).collect();
            json!({ "path": feature.path, "feature": feature.parsed.feature, "background": background, "scenarios": scenarios })
        })
        .collect();
    Ok(format!("{}\n", serde_json::to_string_pretty(&json!({ "version": 1, "features": features }))?))
}

fn then_steps(feature: &Feature) -> Vec<&Step> {
    let mut thens = Vec::new();
    for steps in std::iter::once(&feature.parsed.background)
        .chain(feature.parsed.scenarios.iter().map(|scenario| &scenario.steps))
    {
        let mut then = false;
        for step in steps {
            then = match step.keyword.as_str() {
                "And" | "But" | "*" => then,
                keyword => keyword == "Then",
            };
            if then {
                thens.push(step);
            }
        }
    }
    thens
}

fn suite_offenses(workspace: &Workspace, member: &Member) -> Result<Vec<String>> {
    let dir = member.root.join("features");
    let features = features(&dir)?;
    let mut offenses = Vec::new();
    let ir_path = dir.join("ir.json");
    let committed = std::fs::read_to_string(&ir_path).unwrap_or_default();
    if committed != ir(&features)? {
        offenses.push(format!("{} does not match the feature files", workspace.shown(&ir_path)));
    }
    let code = step_code(member)?;
    if code.marks_wip {
        offenses.push(format!("{}: the steps mark a feature or scenario wip", member.name));
    }
    let bound = then_patterns(&code)?;
    for feature in &features {
        for step in then_steps(feature) {
            offenses.extend(
                then_offense(&bound, &code, step)
                    .map(|problem| format!("{}/{}:{}: {problem}", workspace.shown(&dir), feature.path, step.line)),
            );
        }
    }
    Ok(offenses)
}

fn step_code(member: &Member) -> Result<StepCode> {
    let mut code = StepCode::default();
    for file in &member.sources {
        let source = std::fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
        code.read(&source).with_context(|| format!("parse {}", file.display()))?;
    }
    Ok(code)
}

fn then_patterns(code: &StepCode) -> Result<Vec<(Regex, String)>> {
    code.thens
        .iter()
        .map(|(pattern, step)| {
            Ok((Regex::new(pattern).with_context(|| format!("the step pattern {pattern:?}"))?, step.clone()))
        })
        .collect()
}

fn then_offense(bound: &[(Regex, String)], code: &StepCode, step: &Step) -> Option<String> {
    let Some((_, function)) = bound.iter().find(|(pattern, _)| pattern.is_match(&step.text)) else {
        return Some(format!("Then {:?} is bound by no then(…) step", step.text));
    };
    (!code.asserting.get(function).copied().unwrap_or(false))
        .then(|| format!("Then {:?} runs {function}, which asserts nothing", step.text))
}
