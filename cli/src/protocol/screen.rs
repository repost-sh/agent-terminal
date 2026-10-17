use std::fmt;

use serde::{Deserialize, Serialize};

use super::{Name, Switch};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Baseline {
    pub name: String,
    pub lines: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub(crate) struct SnapshotOptions {
    pub rows: Option<RowRange>,
    pub scrollback: Scrollback,
    pub ansi: bool,
    #[serde(default)]
    pub refs: bool,
    #[serde(default)]
    pub squeeze_blank: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Squeeze {
    Off,
    On,
    PerStep(Vec<Squeeze>),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RowRange {
    pub first: u16,
    pub last: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Scrollback {
    #[default]
    None,
    All,
    Last(usize),
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct SnapshotDiff {
    pub since: DiffBase,
    pub frame: u64,
    pub changes: Vec<RowChange>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiffBase {
    Frame(u64),
    Baseline(String),
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct RowChange {
    pub row: u16,
    pub before: String,
    pub after: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Matches {
    pub matches: Vec<Match>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Match {
    pub row: i32,
    pub col: u16,
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct Snapshot {
    pub session: Name,
    pub tab: Name,
    pub program: String,
    pub cols: u16,
    pub rows: u16,
    pub alt_screen: Switch,
    pub mouse: Option<String>,
    pub cursor: [u16; 2],
    pub cursor_visible: bool,
    pub frame: u64,
    pub exited: bool,
    pub exit_code: Option<i32>,
    pub first_row: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrollback: Option<Vec<String>>,
    pub lines: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refs: Option<Vec<Ref>>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub nothing_printed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct Ref {
    #[serde(rename = "ref")]
    pub id: String,
    pub kind: RefKind,
    pub label: String,
    pub row: u16,
    pub col: u16,
    pub rows: [u16; 2],
    pub cols: [u16; 2],
    pub frame: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum RefKind {
    Panel,
    Selected,
    Button,
    Link,
    Hint,
    Input,
}

impl fmt::Display for RefKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RefKind::Panel => "panel",
            RefKind::Selected => "selected",
            RefKind::Button => "button",
            RefKind::Link => "link",
            RefKind::Hint => "hint",
            RefKind::Input => "input",
        })
    }
}
