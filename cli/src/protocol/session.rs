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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    pub session: Name,
    pub tab: Option<Name>,
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
    pub current: Option<Name>,
    pub tabs: Vec<TabInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct TabInfo {
    pub tab: Name,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renamed_from: Option<Name>,
    pub program: String,
    pub command: String,
    pub cwd: String,
    pub cols: u16,
    pub rows: u16,
    pub exited: bool,
    pub exit_code: Option<i32>,
    pub control: Owner,
}
