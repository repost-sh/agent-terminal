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
use std::path::PathBuf;
use std::time::Duration;

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

use crate::attach;
use crate::connection::{self, IfNotRunning, Launch};
use crate::doctor;
use crate::mcp;
use crate::native::daemon;
use crate::output;
use crate::protocol::request;
use crate::protocol::{Call, DashboardStopped, Reply, Request, Response, SessionInfo, Sessions, TabId, Target};
use crate::session_id;
use crate::skills;
use crate::upgrade;

pub(crate) use argv::without_global_options;

const ON_ITS_OWN: &str = "this command runs on its own, not inside batch";

#[derive(Parser)]
#[command(name = "agent-terminal", version, about = "Fast terminal automation CLI for AI agents")]
struct Cli {
    #[arg(
        long,
        global = true,
        help = "Use a named session (or AGENT_TERMINAL_SESSION)",
        env = "AGENT_TERMINAL_SESSION",
        default_value = "default"
    )]
    session: String,

    #[arg(long, global = true, help = "JSON output")]
    json: bool,

    #[arg(long, global = true, help = "Give up waiting after this long", value_name = "MS", default_value_t = 25_000)]
    timeout: u64,

    #[arg(long, global = true, help = "Act on this tab instead of the current one", value_name = "ID", value_parser = clap::value_parser!(TabId))]
    tab: Option<TabId>,

    #[arg(
        long,
        global = true,
        help = "Stop an unused daemon after this long: 30s, 10m, 1h or raw ms, 0 = never (set when a call starts the daemon)",
        value_name = "TIME",
        default_value = "1h",
        value_parser = values::parse_duration
    )]
    idle_timeout: Duration,

    #[arg(
        long,
        global = true,
        help = "Where screenshots go when the command names no file (default: ~/.agent-terminal/screenshots)",
        env = "AGENT_TERMINAL_SCREENSHOT_DIR",
        value_name = "PATH"
    )]
    screenshot_dir: Option<PathBuf>,

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
    target: Target,
    dir: Option<PathBuf>,
    timeout: u64,
}

pub(crate) fn run() -> io::Result<i32> {
    let mut parser = Cli::command();
    let argv: Vec<OsString> = std::env::args_os().collect();
    let parsed = argv::parse(&mut parser, &argv)
        .and_then(|mut matches| Cli::from_arg_matches_mut(&mut matches).map_err(|e| e.format(&mut parser)));
    match parsed {
        Ok(cli) => dispatch(cli),
        Err(e) => usage(&e),
    }
}

fn usage(e: &clap::Error) -> io::Result<i32> {
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

fn dispatch(cli: Cli) -> io::Result<i32> {
    let json = cli.json;
    let idle_timeout = cli.idle_timeout;
    let request = match cli.command {
        Command::Tools(tools::ToolCommand::Daemon { action: tools::DaemonAction::Run }) => {
            return daemon_run(idle_timeout);
        }
        Command::Tools(tools::ToolCommand::Skills { action }) => return skills::run(action, json, Cli::command()),
        Command::Tools(tools::ToolCommand::Install { .. }) => return skills::install(json),
        Command::Tools(tools::ToolCommand::Mcp) => return mcp::run(Cli::command()),
        Command::Tools(tools::ToolCommand::Attach { read_only }) => {
            let target = Target { session: cli.session, tab: cli.tab };
            return attach::run(target, read_only, json, &Launch { idle_timeout });
        }
        Command::Tools(tools::ToolCommand::Version) => return output::version(json),
        Command::Tools(tools::ToolCommand::Doctor { fix }) => return doctor::run(fix, json),
        Command::Tools(tools::ToolCommand::Upgrade) => return upgrade::run(json),
        Command::Tools(tools::ToolCommand::Batch(args)) => {
            let defaults = batch::Defaults {
                session: &cli.session,
                tab: cli.tab,
                timeout: cli.timeout,
                screenshot_dir: cli.screenshot_dir.as_deref(),
            };
            batch::request(&args, &defaults)
        }
        Command::Tabs(tabs::TabCommand::Session { action: None }) => {
            return output::session(&cli.session, json).map(|()| 0);
        }
        Command::Tabs(tabs::TabCommand::Session {
            action: Some(tabs::SessionAction::Id { scope: tabs::Scope::Worktree, prefix }),
        }) => return session_id(prefix.as_deref(), json),
        command => request(Cli { command, ..cli }),
    };
    match request {
        Ok(request) => send(request, json, idle_timeout),
        Err(message) => output::refuse(&message, json),
    }
}

fn daemon_run(idle_timeout: Duration) -> io::Result<i32> {
    match daemon::run(idle_timeout) {
        Ok(()) => Ok(0),
        Err(e) => output::fail(&format!("{e:#}"), crate::exit::ERROR),
    }
}

fn session_id(prefix: Option<&str>, json: bool) -> io::Result<i32> {
    let id =
        std::env::current_dir().map_err(anyhow::Error::from).and_then(|cwd| session_id::for_worktree(&cwd, prefix));
    match id {
        Ok(id) => output::session_id(&id, json).map(|()| 0),
        Err(e) => output::refuse(&format!("{e:#}"), json),
    }
}

fn json_asked() -> bool {
    std::env::args_os().skip(1).take_while(|arg| arg != "--").any(|arg| arg == "--json")
}

fn request(cli: Cli) -> Result<Request, String> {
    let context = Context {
        target: Target { session: cli.session, tab: cli.tab },
        dir: cli.screenshot_dir,
        timeout: cli.timeout,
    };
    match cli.command {
        Command::Programs(command) => command.request(context),
        Command::Snapshot(args) => {
            Ok(Request::Snapshot(request::Snapshot { target: context.target, options: args.options() }))
        }
        Command::Tabs(command) => command.request(context),
        Command::Reading(command) => command.request(context),
        Command::Input(command) => command.request(context),
        Command::Wait(args) => args.request(context),
        Command::Expect(args) => Ok(Request::Expect(request::Expect { target: context.target, expect: args.check()? })),
        Command::Tools(command) => command.request(),
    }
}

fn send(request: Request, json: bool, idle_timeout: Duration) -> io::Result<i32> {
    let if_not_running = match &request {
        Request::Stop => IfNotRunning::Answer(Reply::Stopped),
        Request::DashboardStop => IfNotRunning::Answer(Reply::DashboardStopped(DashboardStopped { stopped: false })),
        Request::Sessions => IfNotRunning::Answer(Reply::Sessions(Sessions::default())),
        Request::Tabs(request::Tabs { session }) => IfNotRunning::Answer(Reply::Tabs(SessionInfo::empty(session))),
        _ => IfNotRunning::Start(Launch { idle_timeout }),
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
