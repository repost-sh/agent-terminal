use std::ffi::OsString;
use std::path::Path;

use clap::CommandFactory;

use super::tools::BatchArgs;
use super::{Cli, argv, parsed, request as single};
use crate::batch::NotValid;
use crate::output;
use crate::protocol::request;
use crate::protocol::{BatchStep, Name, Request};

pub(super) struct Defaults<'a> {
    pub(super) session: Option<&'a Name>,
    pub(super) tab: Option<&'a Name>,
    pub(super) timeout: u64,
    pub(super) screenshot_dir: Option<&'a Path>,
}

pub(super) fn request(args: &BatchArgs, defaults: &Defaults<'_>) -> Result<Request, String> {
    let parser = Cli::command()
        .no_binary_name(true)
        .mut_arg("session", |arg| match defaults.session {
            Some(session) => arg.env(None::<&str>).default_value(session.to_string()),
            None => arg.env(None::<&str>),
        })
        .mut_arg("tab", |arg| match defaults.tab {
            Some(tab) => arg.default_value(tab.to_string()),
            None => arg,
        })
        .mut_arg("timeout", |arg| arg.default_value(defaults.timeout.to_string()))
        .mut_arg("screenshot_dir", |arg| match defaults.screenshot_dir {
            Some(dir) => arg.env(None::<&str>).default_value(dir.as_os_str().to_owned()),
            None => arg.env(None::<&str>),
        });
    let mut steps = Vec::new();
    for (index, call) in crate::batch::calls(&args.commands)?.into_iter().enumerate() {
        let request = step(parser.clone(), &call)
            .map_err(|reason| NotValid { index, call: &output::call_line(&call), reason: &reason }.to_string())?;
        steps.push(BatchStep { args: call, request });
    }
    Ok(Request::Batch(request::Batch { steps, bail: args.bail }))
}

fn step(mut parser: clap::Command, call: &[String]) -> Result<Request, String> {
    let argv: Vec<OsString> = call.iter().map(OsString::from).collect();
    let matches = argv::parse(&mut parser, &argv).map_err(|e| e.render().to_string())?;
    single(parsed(matches).map_err(|e| e.render().to_string())?)
}
