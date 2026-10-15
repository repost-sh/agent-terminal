use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

pub(crate) type Hits = BTreeMap<u32, u64>;

pub(crate) struct Lines(BTreeMap<PathBuf, Hits>);

impl Lines {
    pub(crate) fn of(&self, file: &Path) -> Option<&Hits> {
        self.0.get(file)
    }
}

pub(crate) fn read(path: &Path) -> Result<Lines> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    parse(&text).with_context(|| format!("parse {}", path.display()))
}

fn parse(text: &str) -> Result<Lines> {
    let mut files: BTreeMap<PathBuf, Hits> = BTreeMap::new();
    let mut file: Option<PathBuf> = None;
    for (number, line) in (1u32..).zip(text.lines()) {
        if let Some(path) = line.strip_prefix("SF:") {
            let path = PathBuf::from(path);
            file = Some(std::fs::canonicalize(&path).unwrap_or(path));
        } else if let Some(hit) = line.strip_prefix("DA:") {
            let Some(file) = &file else { bail!("line {number}: DA before any SF") };
            let (line, hits) = hit_of(hit).with_context(|| format!("line {number}: {line}"))?;
            *files.entry(file.clone()).or_default().entry(line).or_default() += hits;
        } else if line == "end_of_record" {
            file = None;
        }
    }
    Ok(Lines(files))
}

fn hit_of(hit: &str) -> Result<(u32, u64)> {
    let mut fields = hit.split(',');
    let line = fields.next().unwrap_or_default().parse().context("a line number")?;
    let hits = fields.next().context("a hit count")?.parse().context("a hit count")?;
    Ok((line, hits))
}
