use std::path::PathBuf;

use crate::support::AgentTerminal;

pub(super) struct Screen {
    dir: tempfile::TempDir,
}

impl Screen {
    pub(super) fn open(at: &AgentTerminal, size: &str, bytes: &str, wait: &str) -> Self {
        let dir = crate::support::scratch("screen-");
        let path = dir.path().join("screen");
        std::fs::write(&path, bytes).expect("write the screen");
        let path = path.to_string_lossy().into_owned();
        at.run(&["open", "--size", size, "--wait", wait, "sh", "-c", "cat \"$1\"; read _", "sh", &path]).success();
        Self { dir }
    }

    pub(super) fn file(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
}
