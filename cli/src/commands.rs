use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::connection;
use crate::native::daemon;
use crate::output;
use crate::protocol::{Get, Request, Response, RowRange, Scrollback, Size, SnapshotOptions, Wait};

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
    Snapshot(SnapshotArgs),
    #[command(about = "Close the current tab (--all closes every session)")]
    Close {
        #[arg(long, help = "Close every tab in every session")]
        all: bool,
    },
    #[command(about = "One fact about the terminal, e.g. get text, get cursor, get cell 4 3")]
    Get {
        #[command(subcommand)]
        what: GetCommand,
    },
    #[command(about = "Where the text is on screen (row,col)")]
    Find {
        #[arg(help = "Text to look for")]
        text: String,
        #[arg(long, help = "Treat the text as a regular expression")]
        regex: bool,
        #[arg(long, help = "Also look in the scrollback above the screen (negative rows)")]
        all: bool,
    },
    #[command(hide = true)]
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
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

#[derive(Args)]
struct SnapshotArgs {
    #[arg(long, help = "Only rows A to B, counted from 1 at the top", value_name = "A:B", value_parser = parse_rows)]
    rows: Option<RowRange>,

    #[arg(
        long,
        help = "Add N lines of scrollback above the screen (all of it when N is left out)",
        value_name = "N",
        num_args = 0..=1
    )]
    scrollback: Option<Option<usize>>,

    #[arg(long, help = "Keep colors and styles as ANSI escape codes")]
    ansi: bool,
}

impl SnapshotArgs {
    fn options(self) -> SnapshotOptions {
        SnapshotOptions {
            rows: self.rows,
            scrollback: match self.scrollback {
                None => Scrollback::None,
                Some(None) => Scrollback::All,
                Some(Some(n)) => Scrollback::Last(n),
            },
            ansi: self.ansi,
        }
    }
}

#[derive(Subcommand)]
enum GetCommand {
    #[command(about = "Screen text only, no header")]
    Text,
    #[command(about = "Cursor position as row,col (or hidden)")]
    Cursor,
    #[command(about = "Terminal size as COLSxROWS")]
    Size,
    #[command(about = "Frame number of the current screen")]
    Frame,
    #[command(about = "Name of the program running in the terminal")]
    Program,
    #[command(about = "Exit code of the program, or running")]
    ExitCode,
    #[command(about = "Process id of the program")]
    Pid,
    #[command(about = "Title the program set for its window")]
    Title,
    #[command(about = "Working directory the program last reported (OSC 7), else where it started")]
    Cwd,
    #[command(about = "Alt-screen, mouse tracking, bracketed paste, cursor keys, focus events, keyboard protocol")]
    Modes,
    #[command(about = "One character and its style (fg, bg, bold, inverse, underline)")]
    Cell {
        #[arg(help = "Row, from 1 at the top")]
        row: u16,
        #[arg(help = "Column, from 1 at the left")]
        col: u16,
    },
}

impl GetCommand {
    fn request(self) -> Get {
        match self {
            GetCommand::Text => Get::Text,
            GetCommand::Cursor => Get::Cursor,
            GetCommand::Size => Get::Size,
            GetCommand::Frame => Get::Frame,
            GetCommand::Program => Get::Program,
            GetCommand::ExitCode => Get::ExitCode,
            GetCommand::Pid => Get::Pid,
            GetCommand::Title => Get::Title,
            GetCommand::Cwd => Get::Cwd,
            GetCommand::Modes => Get::Modes,
            GetCommand::Cell { row, col } => Get::Cell { row, col },
        }
    }
}

#[derive(Subcommand)]
enum DaemonAction {
    Run,
    Stop,
}

pub fn run() -> i32 {
    let cli = Cli::parse();
    let session = cli.session;
    let request = match cli.command {
        Command::Daemon { action: DaemonAction::Run } => {
            return match daemon::run() {
                Ok(()) => 0,
                Err(e) => output::fail(&format!("{e:#}"), crate::exit::ERROR),
            };
        }
        Command::Daemon { action: DaemonAction::Stop } => Request::Stop,
        Command::Open(args) => {
            let cwd = match args.cwd.map_or_else(std::env::current_dir, Ok) {
                Ok(cwd) => cwd,
                Err(e) => return output::fail(&format!("current directory: {e}"), crate::exit::ERROR),
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
        Command::Snapshot(args) => Request::Snapshot { session, options: args.options() },
        Command::Close { all } => Request::Close { session, all },
        Command::Get { what } => Request::Get { session, what: what.request() },
        Command::Find { text, regex, all } => Request::Find { session, text, regex, all },
    };

    let is_stop = matches!(request, Request::Stop);
    let response = match connection::send(&request, !is_stop) {
        Ok(Some(response)) => response,
        Ok(None) => return 0,
        Err(e) => return output::fail(&format!("{e:#}"), crate::exit::ERROR),
    };
    match response {
        Response::Err(error) => output::error(&error, cli.json),
        Response::Ok(reply) => {
            output::reply(&reply, cli.json);
            0
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

fn parse_rows(value: &str) -> Result<RowRange, String> {
    let (first, last) = value.split_once(':').ok_or("expected A:B, e.g. 1:10")?;
    let first: u16 = first.parse().map_err(|_| format!("invalid row: {first}"))?;
    let last: u16 = last.parse().map_err(|_| format!("invalid row: {last}"))?;
    if first == 0 || first > last {
        return Err("rows count from 1, and A must not be after B".into());
    }
    Ok(RowRange { first, last })
}

fn parse_env(value: &str) -> Result<(String, String), String> {
    let (key, value) = value.split_once('=').ok_or("expected K=V")?;
    if key.is_empty() {
        return Err("expected K=V".into());
    }
    Ok((key.to_string(), value.to_string()))
}
