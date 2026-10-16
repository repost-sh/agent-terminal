use std::fmt;

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
