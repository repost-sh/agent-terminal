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
