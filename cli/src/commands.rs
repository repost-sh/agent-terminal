use std::path::PathBuf;

use clap::{Args, CommandFactory, Parser, Subcommand};

use crate::connection;
use crate::key::Key;
use crate::native::daemon;
use crate::output;
use crate::protocol::{Input, Request, Response, Signal, Size, Wait};
use crate::skills;

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
    #[command(about = "Type text, key by key")]
    Type(TypeArgs),
    #[command(about = "Press keys: Enter, Tab, Esc, Up, F5, Ctrl+c, Alt+x, Shift+Tab; several keys run in order")]
    Press(PressArgs),
    #[command(about = "Paste text (bracketed paste when the app supports it)")]
    Paste(PasteArgs),
    #[command(about = r"Raw bytes with escapes, last resort: \e \xHH \n \r \t \0 \\")]
    Send(SendArgs),
    #[command(about = "Send a signal to the program in the foreground: INT, TERM, KILL, HUP, TSTP, CONT, WINCH")]
    Signal(SignalArgs),
    #[command(about = "Guides that ship with this version (core, tui-qa, cli-qa)")]
    Skills {
        #[command(subcommand)]
        action: Option<skills::Action>,
    },
    #[command(about = "Install the agent skill stub into ~/.claude/skills")]
    Install {
        #[arg(long, help = "Install the agent skill stub (the default)")]
        skills: bool,
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
struct TypeArgs {
    #[arg(help = "The text; a newline presses Enter, a tab presses Tab", allow_hyphen_values = true)]
    text: String,

    #[arg(long, help = "Pause this long between keys", value_name = "MS", default_value_t = 0)]
    delay: u64,

    #[command(flatten)]
    after: AfterInput,
}

#[derive(Args)]
struct PressArgs {
    #[arg(
        help = "Keys: Enter, Tab, Esc, Backspace, Delete, Insert, Up, Down, Left, Right, Home, End, PageUp, PageDown, F1-F12, Space or a character, with Ctrl+, Alt+, Shift+",
        required = true,
        value_name = "KEY",
        value_parser = clap::value_parser!(Key)
    )]
    keys: Vec<Key>,

    #[arg(long, help = "Press the whole sequence this many times", value_name = "N", default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
    repeat: u32,

    #[command(flatten)]
    after: AfterInput,
}

#[derive(Args)]
struct PasteArgs {
    #[arg(help = "The text, pasted in one piece", allow_hyphen_values = true)]
    text: String,

    #[command(flatten)]
    after: AfterInput,
}

#[derive(Args)]
struct SendArgs {
    #[arg(
        help = "The bytes, with escapes for what you cannot type",
        value_name = "BYTES",
        value_parser = parse_bytes,
        allow_hyphen_values = true
    )]
    bytes: RawBytes,

    #[command(flatten)]
    after: AfterInput,
}

#[derive(Args)]
struct SignalArgs {
    #[arg(
        help = "INT, TERM, KILL, HUP, TSTP, CONT or WINCH",
        value_name = "SIGNAL",
        value_parser = clap::value_parser!(Signal)
    )]
    signal: Signal,

    #[command(flatten)]
    after: AfterInput,
}

#[derive(Args)]
struct AfterInput {
    #[arg(long, help = "Then wait until this text is on screen", value_name = "TEXT")]
    wait: Option<String>,
}

#[derive(Clone)]
struct RawBytes(Vec<u8>);

#[derive(Subcommand)]
enum DaemonAction {
    Run,
    Stop,
}

pub fn run() -> i32 {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let code = if e.use_stderr() { crate::exit::ERROR } else { 0 };
            if let Err(print) = e.print() {
                eprintln!("error: printing the usage failed: {print}");
            }
            return code;
        }
    };
    let session = cli.session;
    let request = match cli.command {
        Command::Daemon { action: DaemonAction::Run } => {
            return match daemon::run() {
                Ok(()) => 0,
                Err(e) => output::fail(&format!("{e:#}"), crate::exit::ERROR),
            };
        }
        Command::Daemon { action: DaemonAction::Stop } => Request::Stop,
        Command::Skills { action } => return skills::run(action, cli.json, Cli::command()),
        Command::Install { .. } => return skills::install(cli.json),
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
        Command::Snapshot => Request::Snapshot { session },
        Command::Close { all } => Request::Close { session, all },
        Command::Type(args) => {
            input(session, Input::Type { text: args.text, delay_ms: args.delay }, args.after, cli.timeout)
        }
        Command::Press(args) => {
            input(session, Input::Press { keys: args.keys, repeat: args.repeat }, args.after, cli.timeout)
        }
        Command::Paste(args) => input(session, Input::Paste { text: args.text }, args.after, cli.timeout),
        Command::Send(args) => input(session, Input::Send { bytes: args.bytes.0 }, args.after, cli.timeout),
        Command::Signal(args) => input(session, Input::Signal { signal: args.signal }, args.after, cli.timeout),
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

fn input(session: String, input: Input, after: AfterInput, timeout_ms: u64) -> Request {
    Request::Input { session, input, wait: after.wait.map(|text| Wait::Text { text }), timeout_ms }
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

fn parse_bytes(value: &str) -> Result<RawBytes, String> {
    let mut bytes = Vec::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            bytes.extend_from_slice(c.encode_utf8(&mut [0u8; 4]).as_bytes());
            continue;
        }
        match chars.next() {
            Some('e') => bytes.push(0x1b),
            Some('n') => bytes.push(b'\n'),
            Some('r') => bytes.push(b'\r'),
            Some('t') => bytes.push(b'\t'),
            Some('0') => bytes.push(0),
            Some('\\') => bytes.push(b'\\'),
            Some('x') => {
                let hex: String = chars.by_ref().take(2).collect();
                let byte = Some(&hex)
                    .filter(|hex| hex.len() == 2 && hex.chars().all(|c| c.is_ascii_hexdigit()))
                    .and_then(|hex| u8::from_str_radix(hex, 16).ok())
                    .ok_or_else(|| format!(r"\x needs two hex digits, got \x{hex}"))?;
                bytes.push(byte);
            }
            Some(other) => return Err(format!(r"unknown escape \{other}; use \e, \xHH, \n, \r, \t, \0 or \\")),
            None => return Err(r"a trailing \ escapes nothing; write \\ for a backslash".into()),
        }
    }
    Ok(RawBytes(bytes))
}
