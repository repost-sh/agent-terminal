use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{
    Attached, CommandOutput, Delivered, ErrorBody, Fact, History, Matches, Name, Screenshot, ScreenshotDiff,
    SessionInfo, Sessions, Snapshot, SnapshotDiff,
};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) enum Response {
    Ok(Reply),
    Err(ErrorBody),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "reply", content = "data")]
pub(crate) enum Reply {
    Created(Created),
    Snapshot(Snapshot),
    Closed(Closed),
    Sessions(Sessions),
    Tabs(SessionInfo),
    Delivered(Delivered),
    Waited(WaitOutcome),
    Checked(Checked),
    Stopped,
    KeptRunning(KeptRunning),
    Batch(Vec<BatchOutcome>),
    Fact(Fact),
    Matches(Matches),
    Marked(Marked),
    Printed(Printed),
    Diff(SnapshotDiff),
    Exported(Export),
    Dashboard(Dashboard),
    DashboardStopped(DashboardStopped),
    History(History),
    Command(CommandOutput),
    Screenshot(Screenshot),
    ScreenshotDiff(ScreenshotDiff),
    Attached(Attached),
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Created {
    pub session: Name,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Dashboard {
    pub url: String,
    pub port: u16,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct DashboardStopped {
    pub stopped: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct KeptRunning {
    pub terminals: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Export {
    pub session: String,
    pub path: PathBuf,
    pub files: usize,
    pub bytes: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Marked {
    pub mark: String,
    pub frame: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Printed {
    pub output: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Closed {
    pub closed: Vec<TabRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ended: Vec<Name>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct WaitOutcome {
    pub session: String,
    pub tab: String,
    pub until: String,
    pub waited_ms: u64,
    pub frame: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Checked {
    pub session: String,
    pub tab: String,
    pub fact: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct TabRef {
    pub session: String,
    pub tab: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct BatchOutcome {
    pub args: Vec<String>,
    pub response: Option<Response>,
}

impl Response {
    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            Response::Ok(reply) => reply.exit_code(),
            Response::Err(error) => error.code,
        }
    }
}

impl Reply {
    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            Reply::Batch(outcomes) => outcomes
                .iter()
                .filter_map(|outcome| outcome.response.as_ref())
                .map(Response::exit_code)
                .find(|&code| code != 0)
                .unwrap_or(0),
            Reply::Command(ran) if ran.check => ran.record.exit_code.unwrap_or(crate::exit::ERROR),
            Reply::ScreenshotDiff(diff) if !diff.passed => crate::exit::ASSERTION,
            _ => 0,
        }
    }
}
