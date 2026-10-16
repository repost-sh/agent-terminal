mod attach;
mod batch;
mod commands;
mod connection;
mod doctor;
mod exit;
mod failure;
mod global_options;
mod home;
mod key;
mod mcp;
mod native;
mod output;
mod paint;
mod protocol;
mod skills;
mod upgrade;
mod version;

use std::process::ExitCode;

use anyhow::Context;

fn main() -> anyhow::Result<ExitCode> {
    let code = commands::run().context("write the output")?;
    Ok(exit::status(code))
}
