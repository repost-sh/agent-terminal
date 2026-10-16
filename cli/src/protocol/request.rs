use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{Baseline, Shot, Size, SnapshotOptions, TabId, Target};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Call {
    pub typed: Option<Vec<String>>,
    pub request: Request,
}

pub(crate) enum Addressed<'a> {
    Nothing,
    Session(&'a str),
    Tab(&'a str, Option<TabId>),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "command", rename_all = "kebab-case")]
pub(crate) enum Request {
    Open(Open),
    Snapshot(Snapshot),
    Close(Close),
    Resize(Resize),
    Restart(Restart),
    Sessions,
    Tabs(Tabs),
    SwitchTab(SwitchTab),
    Input(Deliver),
    Wait(WaitFor),
    Expect(Expect),
    Stop,
    StopIfNoTerminals,
    Batch(Batch),
    Get(Get),
    Find(Find),
    Mark(Mark),
    Output(Output),
    DiffSnapshot(DiffSnapshot),
    TraceExport(TraceExport),
    DashboardStart(DashboardStart),
    DashboardStop,
    Shell(OpenShell),
    Exec(Exec),
    History(History),
    HistoryShow(HistoryShow),
    Screenshot(Screenshot),
    DiffScreenshot(DiffScreenshot),
    WaitControl(WaitControl),
    Attach(Attach),
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Open {
    pub session: String,
    pub program: String,
    pub args: Vec<String>,
    pub size: Size,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub wait: Option<super::Wait>,
    pub timeout_ms: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Snapshot {
    #[serde(flatten)]
    pub target: Target,
    #[serde(default)]
    pub options: SnapshotOptions,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Close {
    #[serde(flatten)]
    pub target: Target,
    pub scope: Closing,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Closing {
    Tab,
    Session,
    EverySession,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Resize {
    #[serde(flatten)]
    pub target: Target,
    pub size: Size,
    pub wait: super::Wait,
    pub timeout_ms: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Restart {
    #[serde(flatten)]
    pub target: Target,
    pub wait: super::Wait,
    pub timeout_ms: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Tabs {
    pub session: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct SwitchTab {
    pub session: String,
    pub tab: TabId,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Deliver {
    #[serde(flatten)]
    pub target: Target,
    pub input: super::Input,
    pub wait: Option<super::Wait>,
    #[serde(default)]
    pub snapshot: bool,
    #[serde(default)]
    pub screenshot: Option<Shot>,
    pub timeout_ms: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct WaitFor {
    #[serde(flatten)]
    pub target: Target,
    pub wait: super::Wait,
    pub timeout_ms: u64,
    #[serde(default)]
    pub assert: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Expect {
    #[serde(flatten)]
    pub target: Target,
    pub expect: super::Expect,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Batch {
    pub steps: Vec<BatchStep>,
    pub bail: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Get {
    #[serde(flatten)]
    pub target: Target,
    pub what: super::Get,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Find {
    #[serde(flatten)]
    pub target: Target,
    pub text: String,
    pub regex: bool,
    pub all: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Mark {
    #[serde(flatten)]
    pub target: Target,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Output {
    #[serde(flatten)]
    pub target: Target,
    pub since: Option<String>,
    pub raw: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct DiffSnapshot {
    #[serde(flatten)]
    pub target: Target,
    pub baseline: Option<Baseline>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct TraceExport {
    pub session: String,
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct DashboardStart {
    pub port: u16,
    #[serde(default)]
    pub origins: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct OpenShell {
    pub session: String,
    pub shell: super::Shell,
    pub program: String,
    pub size: Size,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub wait: Option<super::Wait>,
    pub timeout_ms: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Exec {
    #[serde(flatten)]
    pub target: Target,
    pub line: String,
    pub check: bool,
    pub timeout_ms: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct History {
    #[serde(flatten)]
    pub target: Target,
    pub last: Option<usize>,
    pub failed: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct HistoryShow {
    #[serde(flatten)]
    pub target: Target,
    pub id: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Screenshot {
    #[serde(flatten)]
    pub target: Target,
    pub shot: Shot,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct DiffScreenshot {
    #[serde(flatten)]
    pub target: Target,
    pub baseline: PathBuf,
    pub shot: Shot,
    pub threshold: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct WaitControl {
    #[serde(flatten)]
    pub target: Target,
    pub timeout_ms: u64,
    #[serde(default)]
    pub assert: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Attach {
    #[serde(flatten)]
    pub target: Target,
    pub read_only: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct BatchStep {
    pub args: Vec<String>,
    pub request: Request,
}

impl Request {
    pub(crate) fn addressed(&self) -> Addressed<'_> {
        match self {
            Request::Sessions
            | Request::Stop
            | Request::StopIfNoTerminals
            | Request::Batch(_)
            | Request::DashboardStart(_)
            | Request::DashboardStop
            | Request::Attach(_) => Addressed::Nothing,
            Request::Open(Open { session, .. })
            | Request::Shell(OpenShell { session, .. })
            | Request::Tabs(Tabs { session })
            | Request::Close(Close {
                target: Target { session, .. },
                scope: Closing::Session | Closing::EverySession,
            })
            | Request::TraceExport(TraceExport { session, .. }) => Addressed::Session(session),
            Request::SwitchTab(SwitchTab { session, tab }) => Addressed::Tab(session, Some(*tab)),
            Request::Snapshot(Snapshot { target, .. })
            | Request::Close(Close { target, .. })
            | Request::Resize(Resize { target, .. })
            | Request::Restart(Restart { target, .. })
            | Request::Input(Deliver { target, .. })
            | Request::Wait(WaitFor { target, .. })
            | Request::Expect(Expect { target, .. })
            | Request::Get(Get { target, .. })
            | Request::Find(Find { target, .. })
            | Request::Mark(Mark { target, .. })
            | Request::Output(Output { target, .. })
            | Request::DiffSnapshot(DiffSnapshot { target, .. })
            | Request::Exec(Exec { target, .. })
            | Request::History(History { target, .. })
            | Request::HistoryShow(HistoryShow { target, .. })
            | Request::Screenshot(Screenshot { target, .. })
            | Request::DiffScreenshot(DiffScreenshot { target, .. })
            | Request::WaitControl(WaitControl { target, .. }) => Addressed::Tab(&target.session, target.tab),
        }
    }
}
