use std::path::PathBuf;

use clap::{ArgGroup, Args, CommandFactory, Parser, Subcommand};

use crate::connection;
use crate::key::Key;
use crate::native::daemon;
use crate::output;
use crate::protocol::{
    Baseline, Expect, Get, Input, Request, Response, RowRange, Scrollback, Signal, Size, SnapshotOptions, Style,
    TextMatch, Wait,
};
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
    #[command(about = "Name this point in the output, to read or clip from later")]
    Mark {
        #[arg(help = "Name of the point, e.g. before-login")]
        name: String,
    },
    #[command(about = "What the program printed since a point (--raw keeps escape codes)")]
    Output {
        #[arg(
            long,
            help = "A mark name or a frame number (default: since the program started)",
            value_name = "MARK|FRAME"
        )]
        since: Option<String>,
        #[arg(long, help = "Keep escape codes and carriage returns as the program sent them")]
        raw: bool,
    },
    #[command(about = "Compare the screen with an earlier one")]
    Diff {
        #[command(subcommand)]
        what: DiffCommand,
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
    #[command(about = "Wait until the text is on screen")]
    Wait(WaitArgs),
    #[command(about = "Fail now (exit 2) unless the screen or the program is as expected")]
    Expect(ExpectArgs),
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
        help = "How long the screen must stay unchanged to count as stable (default 300)",
        value_name = "MS",
        num_args = 0..=1,
        default_missing_value = "300",
        conflicts_with = "wait"
    )]
    wait_stable: Option<u64>,

    #[arg(long, help = "Print the screen right away, without waiting", conflicts_with_all = ["wait", "wait_stable"])]
    no_wait: bool,

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
enum DiffCommand {
    #[command(about = "Rows that changed since the last snapshot (or since a saved one)")]
    Snapshot {
        #[arg(long, help = "Compare with a snapshot saved to this file instead", value_name = "FILE")]
        baseline: Option<PathBuf>,
    },
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
#[command(group(ArgGroup::new("until").required(true).args(["text", "stable", "idle", "exit", "cursor", "frame"])))]
struct WaitArgs {
    #[arg(
        help = "The text to wait for; a plain number pauses that many milliseconds",
        value_name = "TEXT|MS",
        allow_hyphen_values = true
    )]
    text: Option<String>,

    #[arg(long, requires = "text", help = "Treat the text as a regular expression")]
    regex: bool,

    #[arg(long, requires = "text", help = "Wait until the text is gone from the screen")]
    gone: bool,

    #[arg(
        long,
        requires = "text",
        help = "Only look at rows A to B, counted from 1 at the top",
        value_name = "A:B",
        value_parser = parse_rows
    )]
    rows: Option<RowRange>,

    #[arg(
        long,
        help = "Until the screen has not changed for this long (default 300)",
        value_name = "MS",
        num_args = 0..=1,
        default_missing_value = "300"
    )]
    stable: Option<u64>,

    #[arg(
        long,
        help = "Until the program has printed nothing for this long (default 300)",
        value_name = "MS",
        num_args = 0..=1,
        default_missing_value = "300"
    )]
    idle: Option<u64>,

    #[arg(long, help = "Until the program exits")]
    exit: bool,

    #[arg(long, requires = "exit", help = "Fail (exit 2) unless the program exits with this code", value_name = "N")]
    code: Option<i32>,

    #[arg(long, help = "Until the cursor is at this row and column", value_name = "ROW,COL", value_parser = parse_position)]
    cursor: Option<(u16, u16)>,

    #[arg(long, help = "Until the screen reaches this frame number", value_name = "N")]
    frame: Option<u64>,
}

impl WaitArgs {
    fn until(self) -> Wait {
        if let Some(quiet_ms) = self.stable {
            return Wait::Stable { quiet_ms };
        }
        if let Some(quiet_ms) = self.idle {
            return Wait::Idle { quiet_ms };
        }
        if self.exit {
            return Wait::Exit { code: self.code };
        }
        if let Some((row, col)) = self.cursor {
            return Wait::Cursor { row, col };
        }
        if let Some(frame) = self.frame {
            return Wait::Frame { frame };
        }
        let text = self.text.expect("clap requires one thing to wait for");
        let plain = !self.regex && !self.gone && self.rows.is_none();
        match text.parse() {
            Ok(ms) if plain => Wait::Elapsed { ms },
            _ => {
                let text = TextMatch { text, regex: self.regex, rows: self.rows };
                if self.gone { Wait::Gone(text) } else { Wait::Text(text) }
            }
        }
    }
}

#[derive(Args)]
#[command(group(ArgGroup::new("check").required(true).args(["text", "exit_code", "cell"])))]
struct ExpectArgs {
    #[arg(help = "Text that must be on screen", allow_hyphen_values = true)]
    text: Option<String>,

    #[arg(long, requires = "text", help = "Fail if the text is on screen instead")]
    gone: bool,

    #[arg(long, help = "Fail unless the program has exited with this code", value_name = "N")]
    exit_code: Option<i32>,

    #[arg(
        long,
        requires = "style",
        help = "The cell to check, counted from 1,1 at the top left",
        value_name = "ROW,COL",
        value_parser = parse_position
    )]
    cell: Option<(u16, u16)>,

    #[arg(
        long,
        requires = "cell",
        help = "What the cell must look like: bold, inverse, underline, fg=<color>, bg=<color> (comma-separated)",
        value_name = "STYLE",
        value_delimiter = ',',
        value_parser = clap::value_parser!(Style)
    )]
    style: Vec<Style>,
}

impl ExpectArgs {
    fn check(self) -> Expect {
        if let Some(code) = self.exit_code {
            return Expect::ExitCode { code };
        }
        if let Some((row, col)) = self.cell {
            return Expect::Cell { row, col, style: self.style };
        }
        let text = TextMatch::plain(self.text.expect("clap requires one thing to check"));
        if self.gone { Expect::Gone(text) } else { Expect::Text(text) }
    }
}

#[derive(Args)]
#[command(group(ArgGroup::new("then").args(["wait", "wait_gone", "wait_stable"])))]
struct AfterInput {
    #[arg(long, help = "Then wait until this text is on screen", value_name = "TEXT")]
    wait: Option<String>,

    #[arg(long, help = "Then wait until this text is gone from the screen", value_name = "TEXT")]
    wait_gone: Option<String>,

    #[arg(
        long,
        help = "Then wait until the screen has not changed for this long (default 300)",
        value_name = "MS",
        num_args = 0..=1,
        default_missing_value = "300"
    )]
    wait_stable: Option<u64>,

    #[arg(long, help = "Then print the screen")]
    snapshot: bool,
}

impl AfterInput {
    fn wait(self) -> Option<Wait> {
        match (self.wait, self.wait_gone, self.wait_stable) {
            (Some(text), _, _) => Some(Wait::Text(TextMatch::plain(text))),
            (_, Some(text), _) => Some(Wait::Gone(TextMatch::plain(text))),
            (_, _, Some(quiet_ms)) => Some(Wait::Stable { quiet_ms }),
            (None, None, None) => None,
        }
    }
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
                wait: match (args.wait, args.no_wait) {
                    (_, true) => None,
                    (Some(text), false) => Some(Wait::Text(TextMatch::plain(text))),
                    (None, false) => Some(Wait::Stable { quiet_ms: args.wait_stable.unwrap_or(300) }),
                },
                timeout_ms: cli.timeout,
            }
        }
        Command::Snapshot(args) => Request::Snapshot { session, options: args.options() },
        Command::Close { all } => Request::Close { session, all },
        Command::Get { what } => Request::Get { session, what: what.request() },
        Command::Find { text, regex, all } => Request::Find { session, text, regex, all },
        Command::Mark { name } => Request::Mark { session, name },
        Command::Output { since, raw } => Request::Output { session, since, raw },
        Command::Diff { what: DiffCommand::Snapshot { baseline } } => {
            let baseline = match baseline.map(|path| read_baseline(&path)).transpose() {
                Ok(baseline) => baseline,
                Err(message) => return output::fail(&message, crate::exit::ERROR),
            };
            Request::DiffSnapshot { session, baseline }
        }
        Command::Type(args) => {
            input(session, Input::Type { text: args.text, delay_ms: args.delay }, args.after, cli.timeout)
        }
        Command::Press(args) => {
            input(session, Input::Press { keys: args.keys, repeat: args.repeat }, args.after, cli.timeout)
        }
        Command::Paste(args) => input(session, Input::Paste { text: args.text }, args.after, cli.timeout),
        Command::Send(args) => input(session, Input::Send { bytes: args.bytes.0 }, args.after, cli.timeout),
        Command::Signal(args) => input(session, Input::Signal { signal: args.signal }, args.after, cli.timeout),
        Command::Wait(args) => Request::Wait { session, wait: args.until(), timeout_ms: cli.timeout },
        Command::Expect(args) => Request::Expect { session, expect: args.check() },
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
    let snapshot = after.snapshot;
    Request::Input { session, input, wait: after.wait(), snapshot, timeout_ms }
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

fn read_baseline(path: &std::path::Path) -> Result<Baseline, String> {
    let name = path.display().to_string();
    let saved = std::fs::read_to_string(path).map_err(|e| format!("read the baseline {name}: {e}"))?;
    let lines = output::saved_rows(&saved)
        .ok_or_else(|| format!("{name} is not a snapshot; save one with `agent-terminal snapshot > {name}`"))?;
    Ok(Baseline { name, lines })
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

fn parse_position(value: &str) -> Result<(u16, u16), String> {
    let (row, col) = value.split_once(',').ok_or("expected ROW,COL, e.g. 4,3")?;
    let row: u16 = row.parse().map_err(|_| format!("invalid row: {row}"))?;
    let col: u16 = col.parse().map_err(|_| format!("invalid column: {col}"))?;
    if row == 0 || col == 0 {
        return Err("rows and columns count from 1".into());
    }
    Ok((row, col))
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
