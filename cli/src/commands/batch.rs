use std::ffi::OsString;
use std::path::Path;

use clap::{CommandFactory, FromArgMatches};

use super::tools::BatchArgs;
use super::{Cli, argv, request as single};
use crate::output;
use crate::protocol::request;
use crate::protocol::{BatchStep, Request, TabId};

pub(super) struct Defaults<'a> {
    pub(super) session: &'a str,
    pub(super) tab: Option<TabId>,
    pub(super) timeout: u64,
    pub(super) screenshot_dir: Option<&'a Path>,
}

pub(super) fn request(args: &BatchArgs, defaults: &Defaults<'_>) -> Result<Request, String> {
    let parser = Cli::command()
        .no_binary_name(true)
        .mut_arg("session", |arg| arg.env(None::<&str>).default_value(defaults.session.to_string()))
        .mut_arg("tab", |arg| match defaults.tab {
            Some(id) => arg.default_value(id.to_string()),
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
            .map_err(|reason| crate::batch::not_valid(index, &output::call_line(&call), &reason))?;
        steps.push(BatchStep { args: call, request });
    }
    Ok(Request::Batch(request::Batch { steps, bail: args.bail }))
}

fn step(mut parser: clap::Command, call: &[String]) -> Result<Request, String> {
    let argv: Vec<OsString> = call.iter().map(OsString::from).collect();
    let matches = argv::parse(&mut parser, &argv).map_err(|e| e.render().to_string())?;
    single(Cli::from_arg_matches(&matches).map_err(|e| e.render().to_string())?)
}
