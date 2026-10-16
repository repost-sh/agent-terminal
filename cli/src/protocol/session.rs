use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::Name;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Size {
    pub cols: u16,
    pub rows: u16,
}

impl Size {
    pub(crate) fn new(cols: u16, rows: u16) -> Result<Self, String> {
        if !(2..=1000).contains(&cols) || !(2..=1000).contains(&rows) {
            return Err("columns and rows must be between 2 and 1000".into());
        }
        Ok(Self { cols, rows })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(try_from = "String", into = "String")]
pub struct TabId(u32);

impl TabId {
    pub const FIRST: TabId = TabId(1);

    pub fn next(self) -> TabId {
        TabId(self.0 + 1)
    }
}

impl FromStr for TabId {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        text.strip_prefix('t')
            .unwrap_or(text)
            .parse()
            .ok()
            .filter(|id| *id > 0)
            .map(TabId)
            .ok_or_else(|| format!("invalid tab id: {text} (expected t1, t2, ...)"))
    }
}

impl fmt::Display for TabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0)
    }
}

impl TryFrom<String> for TabId {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        text.parse()
    }
}

impl From<TabId> for String {
    fn from(id: TabId) -> String {
        id.to_string()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    pub session: Name,
    pub tab: Option<TabId>,
}

impl Target {
    pub(crate) fn tab(session: &Name, tab: TabId) -> Self {
        Self { session: session.clone(), tab: Some(tab) }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Owner {
    Agent,
    Human,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Shell {
    Zsh,
    Bash,
    Fish,
}

impl Shell {
    pub(crate) const ALL: [Shell; 3] = [Shell::Zsh, Shell::Bash, Shell::Fish];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Shell::Zsh => "zsh",
            Shell::Bash => "bash",
            Shell::Fish => "fish",
        }
    }

    pub(crate) fn of_program(program: &str) -> Option<Shell> {
        let name = std::path::Path::new(program).file_name()?.to_str()?;
        let name = name.strip_prefix('-').unwrap_or(name);
        Shell::ALL.into_iter().find(|shell| shell.name() == name)
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub(crate) struct Sessions {
    pub sessions: Vec<SessionInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct SessionInfo {
    pub session: Name,
    pub current: Option<String>,
    pub tabs: Vec<TabInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct TabInfo {
    pub tab: String,
    pub program: String,
    pub command: String,
    pub cwd: String,
    pub cols: u16,
    pub rows: u16,
    pub exited: bool,
    pub exit_code: Option<i32>,
    pub control: Owner,
}
