use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    workspace_members: Vec<String>,
    workspace_root: PathBuf,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
    manifest_path: PathBuf,
    dependencies: Vec<Dependency>,
}

#[derive(Deserialize, Clone)]
pub(crate) struct Dependency {
    pub(crate) name: String,
    pub(crate) kind: Option<Kind>,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    Normal,
    Dev,
    Build,
}

pub(crate) struct Member {
    pub(crate) name: String,
    pub(crate) root: PathBuf,
    pub(crate) sources: Vec<PathBuf>,
    pub(crate) unit_tests: Vec<PathBuf>,
    pub(crate) tests: Vec<PathBuf>,
    pub(crate) dependencies: Vec<Dependency>,
}

pub(crate) struct Workspace {
    pub(crate) root: PathBuf,
    pub(crate) members: Vec<Member>,
}

impl Workspace {
    pub(crate) fn load() -> Result<Self> {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = Command::new(cargo).args(["metadata", "--no-deps", "--format-version", "1"]).output()?;
        ensure!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr).trim());
        let metadata: Metadata = serde_json::from_slice(&output.stdout).context("parse the metadata")?;
        let packages: Vec<Package> =
            metadata.packages.into_iter().filter(|package| metadata.workspace_members.contains(&package.id)).collect();
        let roots: Vec<PathBuf> = packages.iter().map(Package::root).collect();
        let members = packages
            .iter()
            .map(|package| {
                let others: Vec<&Path> =
                    roots.iter().filter(|root| **root != package.root()).map(PathBuf::as_path).collect();
                let (unit_tests, sources) = rust_files(&package.root().join("src"), &others)?
                    .into_iter()
                    .partition(|file| file.file_name().is_some_and(|name| name == "tests.rs"));
                Ok(Member {
                    name: package.name.clone(),
                    root: package.root(),
                    sources,
                    unit_tests,
                    tests: rust_files(&package.root().join("tests"), &others)?,
                    dependencies: package.dependencies.clone(),
                })
            })
            .collect::<Result<_>>()?;
        Ok(Self { root: metadata.workspace_root, members })
    }

    pub(crate) fn shown(&self, file: &Path) -> String {
        file.strip_prefix(&self.root).unwrap_or(file).display().to_string()
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Normal => "normal",
            Kind::Dev => "dev",
            Kind::Build => "build",
        })
    }
}

impl Package {
    fn root(&self) -> PathBuf {
        self.manifest_path.parent().unwrap_or(&self.manifest_path).to_path_buf()
    }
}

fn rust_files(dir: &Path, others: &[&Path]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return Ok(files);
    }
    for path in entries(dir)?.into_iter().filter(|path| !others.contains(&path.as_path())) {
        if path.is_dir() {
            files.extend(rust_files(&path, others)?);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(std::fs::canonicalize(&path).with_context(|| format!("find where {} is", path.display()))?);
        }
    }
    files.sort();
    Ok(files)
}

fn entries(dir: &Path) -> Result<Vec<PathBuf>> {
    let listed = std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))?;
    listed.map(|entry| Ok(entry.with_context(|| format!("read {}", dir.display()))?.path())).collect()
}
