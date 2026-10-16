use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::policy::{Layer, ROOT};
use crate::offenses::Rule;
use crate::rust::crate_paths;
use crate::workspace::{Member, Workspace};

const CROSSES_A_LAYER: Rule = Rule {
    anchor: "architecture",
    fix: "move what both need into a contract module, or reach it from a layer the policy lets use it",
};

const UNCLASSIFIED: Rule =
    Rule { anchor: "architecture", fix: "give the module a layer in gauntlet/src/architecture/policy.rs" };

struct Source {
    file: PathBuf,
    module: Vec<String>,
}

impl Source {
    fn top(&self) -> &str {
        self.module.first().map_or(ROOT, String::as_str)
    }
}

pub(super) fn offenses(workspace: &Workspace, member: &Member) -> Result<Vec<(String, &'static Rule)>> {
    let src = std::fs::canonicalize(member.root.join("src"))?;
    let sources: Vec<Source> = member
        .sources
        .iter()
        .filter_map(|file| Some(Source { file: file.clone(), module: module_of(file.strip_prefix(&src).ok()?)? }))
        .collect();
    let tops: BTreeSet<&str> = sources.iter().map(Source::top).collect();
    let mut found: Vec<(String, &'static Rule)> = sources
        .iter()
        .filter(|source| source.module.len() < 2 && Layer::of(source.top()).is_none())
        .map(|source| (format!("{}: {} has no layer", workspace.shown(&source.file), source.top()), &UNCLASSIFIED))
        .collect();
    for source in &sources {
        found.extend(crossings(workspace, source, &tops)?);
    }
    Ok(found)
}

fn crossings(workspace: &Workspace, source: &Source, tops: &BTreeSet<&str>) -> Result<Vec<(String, &'static Rule)>> {
    let Some(layer) = Layer::of(source.top()) else {
        return Ok(Vec::new());
    };
    let code = std::fs::read_to_string(&source.file).with_context(|| format!("read {}", source.file.display()))?;
    let paths = crate_paths(&code, &source.module).with_context(|| format!("parse {}", source.file.display()))?;
    let used: BTreeSet<&str> = paths
        .iter()
        .map(|path| path.first().map(String::as_str).filter(|top| tops.contains(top)).unwrap_or(ROOT))
        .filter(|used| *used != source.top())
        .collect();
    Ok(used
        .into_iter()
        .filter_map(|used| {
            let other = Layer::of(used).filter(|other| !layer.may_use(*other))?;
            let offense =
                format!("{}: {} ({layer}) uses {used} ({other})", workspace.shown(&source.file), source.top());
            Some((offense, &CROSSES_A_LAYER))
        })
        .collect())
}

fn module_of(relative: &Path) -> Option<Vec<String>> {
    let mut module: Vec<String> = relative
        .components()
        .map(|component| component.as_os_str().to_str().map(String::from))
        .collect::<Option<_>>()?;
    let file = module.pop()?;
    let stem = file.strip_suffix(".rs")?;
    if module.is_empty() && stem == ROOT {
        return Some(module);
    }
    if stem != "mod" {
        module.push(stem.to_string());
    }
    Some(module)
}
