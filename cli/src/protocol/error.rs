use serde::{Deserialize, Serialize};

use super::Snapshot;

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct ErrorBody {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<Box<Snapshot>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other_version: Option<OtherVersion>,
}

impl ErrorBody {
    pub(crate) fn new(code: i32, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), snapshot: None, other_version: None }
    }

    pub(crate) fn failure(message: impl Into<String>) -> Self {
        Self::new(crate::exit::ERROR, message)
    }

    pub(crate) fn with_screen(self, snapshot: Snapshot) -> Self {
        Self { snapshot: Some(Box::new(snapshot)), ..self }
    }

    pub(crate) fn other_version(message: impl Into<String>, other_version: OtherVersion) -> Self {
        Self { other_version: Some(other_version), ..Self::new(crate::exit::OTHER_VERSION, message) }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct OtherVersion {
    pub cli_version: Option<String>,
    pub daemon_version: Option<String>,
    pub terminals: Option<usize>,
}

pub(crate) fn daemon_stop_advice(cli_version: Option<&str>) -> String {
    let next = match cli_version {
        Some(version) => format!("a {version} daemon"),
        None => "a daemon of the command's version".to_string(),
    };
    format!("`agent-terminal daemon stop` ends it and its terminals, and the next command starts {next}")
}
