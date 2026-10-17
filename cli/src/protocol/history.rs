use serde::{Deserialize, Serialize};

use super::Name;

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct History {
    pub session: Name,
    pub tab: Name,
    pub total: usize,
    pub commands: Vec<CommandRecord>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct CommandOutput {
    pub session: Name,
    pub tab: Name,
    #[serde(flatten)]
    pub record: CommandRecord,
    pub output: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub check: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct CommandRecord {
    pub id: u32,
    pub command: String,
    pub cwd: String,
    pub started_at_ms: u64,
    pub ended_at_ms: Option<u64>,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub first_frame: u64,
    pub last_frame: Option<u64>,
}

impl CommandRecord {
    pub(crate) fn finished(&self) -> bool {
        self.ended_at_ms.is_some()
    }

    pub(crate) fn failed(&self) -> bool {
        self.finished() && self.exit_code.is_some_and(|code| code != 0)
    }
}
