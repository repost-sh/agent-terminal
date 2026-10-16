mod argv;
mod batch;
mod checks;
mod input;
mod programs;
mod reading;
mod tabs;
mod tools;
mod values;

use std::ffi::OsString;
use std::io;

use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::parser::ValueSource;
use clap::{ArgMatches, CommandFactory, FromArgMatches, Parser, Subcommand};

use crate::attach;
use crate::connection::{self, IfNotRunning};
use crate::global_options::GlobalOptions;
use crate::mcp;
use crate::native::daemon;
use crate::output;
use crate::protocol::request;
use crate::protocol::{Call, DashboardStopped, Name, Reply, Request, Response, Sessions, Target};
use crate::skills;
use crate::upgrade;

const ON_ITS_OWN: &str = "this command runs on its own, not inside batch";

#[derive(Parser)]
#[command(name = "agent-terminal", version, about = "Fast terminal automation CLI for AI agents")]
struct Cli {
    #[command(flatten)]
    global: GlobalOptions,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(flatten)]
    Programs(programs::ProgramCommand),
    #[command(about = "The screen as text: header, numbered rows, cursor")]
    Snapshot(reading::SnapshotArgs),
    #[command(flatten)]
    Tabs(tabs::TabCommand),
    #[command(flatten)]
    Reading(reading::ReadCommand),
    #[command(flatten)]
    Input(input::InputCommand),
    #[command(about = "Wait until the text is on screen, the screen is stable, or the program exits")]
    Wait(checks::UntilArgs),
    #[command(about = "Fail now (exit 2) unless the screen or the program is as expected")]
    Expect(checks::ExpectArgs),
    #[command(flatten)]
    Tools(tools::ToolCommand),
}

struct Context {
    global: GlobalOptions,
    session_typed: bool,
}

impl Context {
    fn session(&self) -> Result<Name, String> {
        self.global.session().cloned().map_err(String::from)
    }

    fn target(&self) -> Result<Target, String> {
        Ok(Target { session: self.session()?, tab: self.global.tab.clone() })
    }
}

pub(crate) fn run() -> io::Result<i32> {
    let mut parser = Cli::command();
    let argv: Vec<OsString> = std::env::args_os().collect();
    let parsed = argv::parse(&mut parser, &argv).and_then(|matches| parsed(matches).map_err(|e| e.format(&mut parser)));
    match parsed {
        Ok((command, context)) => dispatch(command, context),
        Err(e) => usage(&e),
    }
}

fn parsed(mut matches: ArgMatches) -> Result<(Command, Context), clap::Error> {
    let session_typed = matches.value_source("session") == Some(ValueSource::CommandLine);
    let Cli { global, command } = Cli::from_arg_matches_mut(&mut matches)?;
    Ok((command, Context { global, session_typed }))
}

fn unnamed_tab(e: &clap::Error) -> bool {
    e.kind() == ErrorKind::MissingRequiredArgument
        && matches!(e.get(ContextKind::InvalidArg), Some(ContextValue::Strings(missing)) if *missing == ["--name <NAME>"])
}

fn usage(e: &clap::Error) -> io::Result<i32> {
    if unnamed_tab(e) {
        return output::refuse(programs::NAME_THE_TAB, json_asked());
    }
    if e.use_stderr() && json_asked() {
        let rendered = e.render().to_string();
        let message = rendered.strip_prefix("error: ").unwrap_or(&rendered).trim_end();
        return output::refuse(message, true);
    }
    let code = if e.use_stderr() { crate::exit::ERROR } else { 0 };
    match e.print() {
        Ok(()) => Ok(code),
        Err(print) => output::fail(&format!("printing the usage failed: {print}"), code),
    }
}

fn dispatch(command: Command, context: Context) -> io::Result<i32> {
    let json = context.global.json;
    let request = match command {
        Command::Tools(tools::ToolCommand::Daemon { action: tools::DaemonAction::Run }) => {
            return daemon_run();
        }
        Command::Tools(tools::ToolCommand::Skills { action }) => {
            return action.unwrap_or(skills::Action::List).run(json, Cli::command());
        }
        Command::Tools(tools::ToolCommand::Mcp) => return mcp::run(Cli::command()),
        Command::Tools(tools::ToolCommand::Attach { read_only }) => {
            return attach::run(read_only, context.global);
        }
        Command::Tools(tools::ToolCommand::Version) => return output::version(json),
        Command::Tools(tools::ToolCommand::Upgrade) => return upgrade::run(json),
        Command::Tools(tools::ToolCommand::Batch(args)) => {
            let global = &context.global;
            let defaults = batch::Defaults {
                session: global.session.as_ref(),
                tab: global.tab.as_ref(),
                timeout: global.timeout,
                screenshot_dir: global.screenshot_dir.as_deref(),
            };
            batch::request(&args, &defaults)
        }
        Command::Tabs(tabs::TabCommand::Session { action: None }) => {
            return match context.global.session() {
                Ok(session) => output::session(session, json).map(|()| 0),
                Err(unnamed) => output::refuse(unnamed, json),
            };
        }
        command => request(command, context),
    };
    match request {
        Ok(request) => send(request, json),
        Err(message) => output::refuse(&message, json),
    }
}

fn daemon_run() -> io::Result<i32> {
    match daemon::run() {
        Ok(()) => Ok(0),
        Err(e) => output::fail(&format!("{e:#}"), crate::exit::ERROR),
    }
}

fn json_asked() -> bool {
    std::env::args_os().skip(1).take_while(|arg| arg != "--").any(|arg| arg == "--json")
}

fn request(command: Command, context: Context) -> Result<Request, String> {
    match command {
        Command::Programs(command) => command.request(&context),
        Command::Snapshot(args) => {
            Ok(Request::Snapshot(request::Snapshot { target: context.target()?, options: args.options() }))
        }
        Command::Tabs(command) => command.request(&context),
        Command::Reading(command) => command.request(context),
        Command::Input(command) => command.request(context),
        Command::Wait(args) => args.request(&context),
        Command::Expect(args) => {
            Ok(Request::Expect(request::Expect { target: context.target()?, expect: args.check()? }))
        }
        Command::Tools(command) => command.request(),
    }
}

fn send(request: Request, json: bool) -> io::Result<i32> {
    let if_not_running = match &request {
        Request::Stop => IfNotRunning::Answer(Box::new(Reply::Stopped)),
        Request::DashboardStop => {
            IfNotRunning::Answer(Box::new(Reply::DashboardStopped(DashboardStopped { stopped: false })))
        }
        Request::Sessions => IfNotRunning::Answer(Box::new(Reply::Sessions(Sessions::default()))),
        _ => IfNotRunning::Start,
    };
    let typed = std::env::args_os().skip(1).map(|arg| arg.to_string_lossy().into_owned()).collect();
    let response = match connection::send(&Call { typed: Some(typed), request }, if_not_running) {
        Ok(response) => response,
        Err(e) => return output::refuse(&format!("{e:#}"), json),
    };
    match response {
        Response::Err(error) => output::error(&error, json),
        Response::Ok(reply) => {
            output::reply(&reply, json)?;
            Ok(reply.exit_code())
        }
    }
}

#[cfg(test)]
mod tests;
