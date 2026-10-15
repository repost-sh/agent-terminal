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

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod exit {
    pub const ERROR: i32 = 1;
    pub const ASSERTION: i32 = 2;
    pub const TIMEOUT: i32 = 3;
    pub const HUMAN_CONTROL: i32 = 4;
    pub const EXITED: i32 = 5;
    pub const OTHER_VERSION: i32 = 6;
}

#[derive(Debug)]
pub struct Error {
    pub code: i32,
    pub message: String,
}

impl Error {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

fn main() {
    std::process::exit(commands::run());
}
