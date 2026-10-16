mod discovery;
mod get;
mod guide_commands;
mod install;
mod path;
mod qa_setup;
mod shipped;
mod stub;

pub(crate) use shipped::shipped;
pub(crate) use stub::{put_stub, stub_in, stub_of_version};
