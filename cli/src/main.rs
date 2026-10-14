mod commands;
mod connection;
mod native;
mod output;
mod protocol;
mod skills;

use std::fmt;

pub mod exit {
    pub const ERROR: i32 = 1;
    pub const TIMEOUT: i32 = 3;
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
