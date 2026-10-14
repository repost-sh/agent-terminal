use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::connection::{self, IfNotRunning, Launch};
use crate::native::daemon;
use crate::output;
use crate::protocol::{Reply, Request, Response, SessionInfo, Sessions, Size, Wait};
use crate::session_id;

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

    #[arg(long, global = true, help = "Act on this tab instead of the current one", value_name = "ID", value_parser = parse_tab)]
    tab: Option<u32>,

    #[arg(
        long,
        global = true,
        help = "Stop an unused daemon after this long: 30s, 10m, 1h or raw ms, 0 = never (set when a call starts the daemon)",
        value_name = "TIME",
        default_value = "1h",
        value_parser = parse_duration
    )]
    idle_timeout: Duration,

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
    #[command(about = "Run the same program again with the same options (prints its first stable screen)")]
    Restart(WaitArgs),
    #[command(about = "Resize the terminal, e.g. resize 80x24 (the app gets SIGWINCH)")]
    Resize {
        #[arg(value_parser = parse_size, value_name = "COLSxROWS")]
        size: Size,
        #[command(flatten)]
        wait: WaitArgs,
    },
    #[command(about = "Current session name (list: every session with its tabs)")]
    Session {
        #[command(subcommand)]
        action: Option<SessionAction>,
    },
    #[command(about = "List tabs in this session (tab <id> switches to one)", args_conflicts_with_subcommands = true)]
    Tab {
        #[arg(help = "Switch to this tab", value_name = "ID", value_parser = parse_tab)]
        id: Option<u32>,
        #[command(subcommand)]
        action: Option<TabAction>,
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

    #[command(flatten)]
    wait: WaitArgs,

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
struct WaitArgs {
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
}

impl WaitArgs {
    fn until(self) -> Wait {
        match self.wait {
            Some(text) => Wait::Text { text },
            None => Wait::Stable { quiet_ms: self.wait_stable },
        }
    }
}

#[derive(Subcommand)]
enum SessionAction {
    #[command(about = "Sessions, their tabs, programs, sizes and status")]
    List,
    #[command(about = "A name unique to this checkout, for AGENT_TERMINAL_SESSION")]
    Id {
        #[arg(long, value_enum, default_value_t = Scope::Worktree, help = "What the name is unique to")]
        scope: Scope,
        #[arg(long, help = "Put this before the name, e.g. the project or task")]
        prefix: Option<String>,
    },
}

#[derive(Subcommand)]
enum TabAction {
    #[command(about = "List tabs in this session")]
    List,
    #[command(about = "Another terminal in the same session (e.g. a server next to the app)")]
    New(OpenArgs),
    #[command(about = "Close a tab (the current one when no id is given)")]
    Close {
        #[arg(value_parser = parse_tab, value_name = "ID")]
        id: Option<u32>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Scope {
    #[value(help = "The git checkout or worktree around the current directory (else the directory itself)")]
    Worktree,
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
            return match daemon::run(cli.idle_timeout) {
                Ok(()) => 0,
                Err(e) => output::fail(&format!("{e:#}"), crate::exit::ERROR),
            };
        }
        Command::Daemon { action: DaemonAction::Stop } => Request::Stop,
        Command::Open(args) | Command::Tab { action: Some(TabAction::New(args)), .. } => {
            match open(session, args, cli.timeout) {
                Ok(request) => request,
                Err(message) => return output::fail(&message, crate::exit::ERROR),
            }
        }
        Command::Tab { id: Some(tab), .. } => Request::SwitchTab { session, tab },
        Command::Tab { id: None, action: None | Some(TabAction::List) } => Request::Tabs { session },
        Command::Tab { action: Some(TabAction::Close { id }), .. } => {
            Request::Close { session, all: false, tab: id.or(cli.tab) }
        }
        Command::Session { action: None } => {
            output::session(&session, cli.json);
            return 0;
        }
        Command::Session { action: Some(SessionAction::List) } => Request::Sessions,
        Command::Session { action: Some(SessionAction::Id { scope: Scope::Worktree, prefix }) } => {
            let id = std::env::current_dir()
                .map_err(anyhow::Error::from)
                .and_then(|cwd| session_id::for_worktree(&cwd, prefix.as_deref()));
            return match id {
                Ok(id) => {
                    output::session_id(&id, cli.json);
                    0
                }
                Err(e) => output::fail(&format!("{e:#}"), crate::exit::ERROR),
            };
        }
        Command::Restart(wait) => {
            Request::Restart { session, tab: cli.tab, wait: wait.until(), timeout_ms: cli.timeout }
        }
        Command::Resize { size, wait } => {
            Request::Resize { session, tab: cli.tab, size, wait: wait.until(), timeout_ms: cli.timeout }
        }
        Command::Snapshot => Request::Snapshot { session, tab: cli.tab },
        Command::Close { all } => Request::Close { session, all, tab: cli.tab },
    };

    let if_not_running = match &request {
        Request::Stop => IfNotRunning::Answer(Reply::Stopped),
        Request::Sessions => IfNotRunning::Answer(Reply::Sessions(Sessions::default())),
        Request::Tabs { session } => IfNotRunning::Answer(Reply::Tabs(SessionInfo::empty(session))),
        _ => IfNotRunning::Start(Launch { idle_timeout: cli.idle_timeout }),
    };
    let response = match connection::send(&request, if_not_running) {
        Ok(response) => response,
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

fn open(session: String, args: OpenArgs, timeout_ms: u64) -> Result<Request, String> {
    let cwd = args.cwd.map_or_else(std::env::current_dir, Ok).map_err(|e| format!("current directory: {e}"))?;
    let mut env: Vec<(String, String)> =
        std::env::vars().filter(|(k, _)| !args.env.iter().any(|(o, _)| o == k)).collect();
    env.extend(args.env);
    let mut command = args.command.into_iter();
    Ok(Request::Open {
        session,
        program: command.next().expect("clap requires a program"),
        args: command.collect(),
        size: args.size,
        cwd,
        env,
        wait: args.wait.until(),
        timeout_ms,
    })
}

fn parse_tab(value: &str) -> Result<u32, String> {
    value
        .strip_prefix('t')
        .unwrap_or(value)
        .parse()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| format!("invalid tab id: {value} (expected t1, t2, ...)"))
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    let refused = || format!("invalid time: {value} (expected a duration like 30s, 10m, 1h or raw ms (0 = never))");
    let unit_at = value.find(|c: char| !c.is_ascii_digit()).unwrap_or(value.len());
    let (number, unit) = value.split_at(unit_at);
    let number: u64 = number.parse().map_err(|_| refused())?;
    let millis_per_unit = match unit {
        "" | "ms" => 1,
        "s" => 1_000,
        "m" => 60_000,
        "h" => 3_600_000,
        _ => return Err(refused()),
    };
    number.checked_mul(millis_per_unit).map(Duration::from_millis).ok_or_else(refused)
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
