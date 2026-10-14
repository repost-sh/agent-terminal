use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize, Debug)]
pub struct SessionId {
    pub session: String,
    pub scope: &'static str,
    pub path: String,
    pub hash: String,
}

pub fn for_worktree(cwd: &Path, prefix: Option<&str>) -> Result<SessionId> {
    let cwd = cwd.canonicalize().with_context(|| format!("resolve {}", cwd.display()))?;
    let root = cwd.ancestors().find(|dir| dir.join(".git").exists()).unwrap_or(&cwd);
    let path = root.to_string_lossy().into_owned();
    let hash: String = Sha256::digest(path.as_bytes()).iter().take(6).map(|byte| format!("{byte:02x}")).collect();
    let session = match prefix.filter(|prefix| !prefix.is_empty()) {
        Some(prefix) => format!("{prefix}-{hash}"),
        None => hash.clone(),
    };
    Ok(SessionId { session, scope: "worktree", path, hash })
}
