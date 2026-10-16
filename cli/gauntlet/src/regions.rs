use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

const CODE: u64 = 0;

#[derive(Deserialize)]
struct Export {
    data: Vec<Data>,
}

#[derive(Deserialize)]
struct Data {
    functions: Vec<Compiled>,
}

#[derive(Deserialize)]
struct Compiled {
    filenames: Vec<PathBuf>,
    regions: Vec<[u64; 8]>,
}

#[derive(Clone, Copy)]
pub(crate) struct Region {
    pub(crate) line: u32,
    pub(crate) ran: bool,
}

pub(crate) struct Regions(BTreeMap<PathBuf, Vec<Region>>);

impl Regions {
    pub(crate) fn of(&self, file: &Path) -> &[Region] {
        self.0.get(file).map_or(&[], Vec::as_slice)
    }
}

type Counted = (PathBuf, [u64; 4], u64);

fn code_regions(compiled: &Compiled) -> impl Iterator<Item = Counted> + '_ {
    let files: Vec<PathBuf> =
        compiled.filenames.iter().map(|name| std::fs::canonicalize(name).unwrap_or_else(|_| name.clone())).collect();
    compiled.regions.iter().filter_map(move |&[line, column, end_line, end_column, count, file, _, kind]| {
        let file = files.get(usize::try_from(file).ok()?)?;
        (kind == CODE).then(|| (file.clone(), [line, column, end_line, end_column], count))
    })
}

pub(crate) fn read(path: &Path) -> Result<Regions> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let export: Export = serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    let mut counted: BTreeMap<(PathBuf, [u64; 4]), u64> = BTreeMap::new();
    for (file, at, count) in export.data.iter().flat_map(|data| &data.functions).flat_map(code_regions) {
        let most = counted.entry((file, at)).or_default();
        *most = (*most).max(count);
    }
    let mut files: BTreeMap<PathBuf, Vec<Region>> = BTreeMap::new();
    for ((file, [line, ..]), count) in counted {
        let line = u32::try_from(line).with_context(|| format!("line {line} of {}", file.display()))?;
        files.entry(file).or_default().push(Region { line, ran: count > 0 });
    }
    Ok(Regions(files))
}
