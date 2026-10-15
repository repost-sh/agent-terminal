mod attach;
mod batch;
mod commands;
mod connection;
mod doctor;
mod key;
mod mcp;
mod native;
mod output;
mod paint;
mod protocol;
mod session_id;
mod skills;
mod upgrade;

use std::fmt;
use std::process::ExitCode;

use anyhow::Context;

pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) fn this_version() -> Result<semver::Version, semver::Error> {
    semver::Version::parse(VERSION)
}

pub(crate) mod exit {
    use std::process::ExitCode;

    pub(crate) const ERROR: i32 = 1;
    pub(crate) const ASSERTION: i32 = 2;
    pub(crate) const TIMEOUT: i32 = 3;
    pub(crate) const HUMAN_CONTROL: i32 = 4;
    pub(crate) const EXITED: i32 = 5;
    pub(crate) const OTHER_VERSION: i32 = 6;

    pub(crate) fn status(code: i32) -> ExitCode {
        let [low, ..] = code.to_le_bytes();
        ExitCode::from(low)
    }
}

#[derive(Debug)]
pub(crate) struct Failure {
    pub code: i32,
    pub message: String,
}

impl Failure {
    pub(crate) fn new(code: i32, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Failure {}

fn main() -> anyhow::Result<ExitCode> {
    let code = commands::run().context("write the output")?;
    Ok(exit::status(code))
}
