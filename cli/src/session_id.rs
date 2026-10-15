use std::fmt;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize, Debug)]
pub(crate) struct SessionId {
    pub session: String,
    pub scope: &'static str,
    pub path: String,
    pub hash: String,
}

pub(crate) fn for_worktree(cwd: &Path, prefix: Option<&str>) -> Result<SessionId> {
    let cwd = cwd.canonicalize().with_context(|| format!("resolve {}", cwd.display()))?;
    let root = cwd.ancestors().find(|dir| dir.join(".git").exists()).unwrap_or(&cwd);
    let path = root.to_string_lossy().into_owned();
    let digest = Sha256::digest(path.as_bytes());
    let hash = fmt::from_fn(|f| digest.iter().take(6).try_for_each(|byte| write!(f, "{byte:02x}"))).to_string();
    let session = match prefix.filter(|prefix| !prefix.is_empty()) {
        Some(prefix) => format!("{prefix}-{hash}"),
        None => hash.clone(),
    };
    Ok(SessionId { session, scope: "worktree", path, hash })
}
