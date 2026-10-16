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
}

pub(crate) struct Member {
    pub(crate) name: String,
    pub(crate) root: PathBuf,
    pub(crate) sources: Vec<PathBuf>,
    pub(crate) tests: Vec<PathBuf>,
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
                Ok(Member {
                    name: package.name.clone(),
                    root: package.root(),
                    sources: rust_files(&package.root().join("src"), &others)?,
                    tests: rust_files(&package.root().join("tests"), &others)?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(Self { root: metadata.workspace_root, members })
    }

    pub(crate) fn shown(&self, file: &Path) -> String {
        file.strip_prefix(&self.root).unwrap_or(file).display().to_string()
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
    for entry in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let path = entry?.path();
        if others.contains(&path.as_path()) {
            continue;
        }
        if path.is_dir() {
            files.extend(rust_files(&path, others)?);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(std::fs::canonicalize(&path)?);
        }
    }
    files.sort();
    Ok(files)
}
