use std::path::PathBuf;

use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};

use crate::batch;
use crate::connection;
use crate::doctor;
use crate::native::daemon;
use crate::output;
use crate::protocol::{BatchStep, Request, Response, Size, Wait};
use crate::upgrade;

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

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Run a program in a new terminal (prints the first stable screen)")]
    Open(OpenArgs),
    #[command(about = "The screen as text: header, numbered rows, cursor")]
    Snapshot,
    #[command(about = "Close the current tab (--all closes every session)")]
    Close {
        #[arg(long, help = "Close every tab in every session")]
        all: bool,
    },
    #[command(hide = true)]
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    #[command(
        about = "Several commands in one call (args or stdin)",
        long_about = "Run several commands in order in one call to the daemon.\n\nPass each command as one quoted argument, e.g. batch \"open htop\" \"snapshot\", or pipe a JSON array to stdin whose items are command lines or argument arrays, e.g. [[\"open\", \"htop\"], \"snapshot\"]. Commands use the batch's --session and --timeout unless they set their own. Every command is parsed before any of them runs. Each command's output and exit code are printed in order, and batch exits with the code of the first command that failed."
    )]
    Batch(BatchArgs),
    #[command(about = "Check PTY support and the daemon socket, and say how to fix each problem")]
    Doctor {
        #[arg(long, help = "Remove stale sockets and state")]
        fix: bool,
    },
    #[command(about = "Upgrade to the latest version, with npm or Homebrew, whichever installed it")]
    Upgrade,
    #[command(about = "Print the version")]
    Version,
}

#[derive(Args)]
struct BatchArgs {
    #[arg(long, help = "Stop at the first command that fails")]
    bail: bool,

    #[arg(help = "The commands, each as one quoted argument", value_name = "COMMAND")]
    commands: Vec<String>,
}

#[derive(Args)]
struct OpenArgs {
    #[arg(long, help = "Terminal size", value_name = "COLSxROWS", default_value = "120x40", value_parser = parse_size)]
    size: Size,

    #[arg(long, help = "Working directory (default: the current directory)")]
    cwd: Option<PathBuf>,

    #[arg(long = "env", help = "Extra environment variable (repeatable)", value_name = "K=V", value_parser = parse_env)]
    env: Vec<(String, String)>,

    #[arg(long, help = "Wait until this text is on screen, not just until the screen is stable", value_name = "TEXT")]
    wait: Option<String>,

    #[arg(
        long,
        help = "How long the screen must stay unchanged to count as stable",
        value_name = "MS",
        default_value_t = 300,
        conflicts_with = "wait"
    )]
    wait_stable: u64,

    #[arg(
        help = "The program and its arguments",
        required = true,
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "PROGRAM"
    )]
    command: Vec<String>,
}

#[derive(Subcommand)]
enum DaemonAction {
    Run,
    Stop,
}

pub fn run() -> i32 {
    let cli = Cli::parse();
    let json = cli.json;
    let request = match &cli.command {
        Command::Daemon { action: DaemonAction::Run } => {
            return match daemon::run() {
                Ok(()) => 0,
                Err(e) => output::fail(&format!("{e:#}"), crate::exit::ERROR),
            };
        }
        Command::Version => return output::version(json),
        Command::Doctor { fix } => return doctor::run(*fix, json),
        Command::Upgrade => return upgrade::run(json),
        Command::Batch(args) => batch(args, &cli.session, cli.timeout),
        _ => request(cli),
    };
    match request {
        Ok(request) => send(&request, json),
        Err(message) => output::fail(&message, crate::exit::ERROR),
    }
}

fn request(cli: Cli) -> Result<Request, String> {
    let session = cli.session;
    let request = match cli.command {
        Command::Daemon { action: DaemonAction::Stop } => Request::Stop,
        Command::Open(args) => {
            let cwd = match args.cwd.map_or_else(std::env::current_dir, Ok) {
                Ok(cwd) => cwd,
                Err(e) => return Err(format!("current directory: {e}")),
            };
            let mut env: Vec<(String, String)> =
                std::env::vars().filter(|(k, _)| !args.env.iter().any(|(o, _)| o == k)).collect();
            env.extend(args.env);
            let mut command = args.command.into_iter();
            Request::Open {
                session,
                program: command.next().expect("clap requires a program"),
                args: command.collect(),
                size: args.size,
                cwd,
                env,
                wait: match args.wait {
                    Some(text) => Wait::Text { text },
                    None => Wait::Stable { quiet_ms: args.wait_stable },
                },
                timeout_ms: cli.timeout,
            }
        }
        Command::Snapshot => Request::Snapshot { session },
        Command::Close { all } => Request::Close { session, all },
        Command::Daemon { action: DaemonAction::Run }
        | Command::Batch(_)
        | Command::Doctor { .. }
        | Command::Upgrade
        | Command::Version => {
            return Err("this command runs on its own, not inside batch".into());
        }
    };

    Ok(request)
}

fn batch(args: &BatchArgs, session: &str, timeout: u64) -> Result<Request, String> {
    let parser = Cli::command()
        .no_binary_name(true)
        .mut_arg("session", |arg| arg.env(None::<&str>).default_value(session.to_string()))
        .mut_arg("timeout", |arg| arg.default_value(timeout.to_string()));
    let mut steps = Vec::new();
    for (index, call) in batch::calls(&args.commands)?.into_iter().enumerate() {
        let request = batch_step(parser.clone(), &call)
            .map_err(|reason| batch::not_valid(index, &output::call_line(&call), &reason))?;
        steps.push(BatchStep { args: call, request });
    }
    Ok(Request::Batch { steps, bail: args.bail })
}

fn batch_step(parser: clap::Command, call: &[String]) -> Result<Request, String> {
    let matches = parser.try_get_matches_from(call).map_err(|e| e.render().to_string())?;
    request(Cli::from_arg_matches(&matches).map_err(|e| e.render().to_string())?)
}

fn send(request: &Request, json: bool) -> i32 {
    let is_stop = matches!(request, Request::Stop);
    let response = match connection::send(request, !is_stop) {
        Ok(Some(response)) => response,
        Ok(None) => return 0,
        Err(e) => return output::fail(&format!("{e:#}"), crate::exit::ERROR),
    };
    match response {
        Response::Err(error) => output::error(&error, json),
        Response::Ok(reply) => {
            output::reply(&reply, json);
            reply.exit_code()
        }
    }
}

fn parse_size(value: &str) -> Result<Size, String> {
    let (cols, rows) = value.split_once('x').ok_or("expected COLSxROWS, e.g. 120x40")?;
    let cols: u16 = cols.parse().map_err(|_| format!("invalid column count: {cols}"))?;
    let rows: u16 = rows.parse().map_err(|_| format!("invalid row count: {rows}"))?;
    if !(2..=1000).contains(&cols) || !(2..=1000).contains(&rows) {
        return Err("columns and rows must be between 2 and 1000".into());
    }
    Ok(Size { cols, rows })
}

fn parse_env(value: &str) -> Result<(String, String), String> {
    let (key, value) = value.split_once('=').ok_or("expected K=V")?;
    if key.is_empty() {
        return Err("expected K=V".into());
    }
    Ok((key.to_string(), value.to_string()))
}
