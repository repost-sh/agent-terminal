use std::path::PathBuf;
use std::time::Duration;

use clap::{ArgGroup, Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};

use crate::batch;
use crate::connection::{self, IfNotRunning, Launch};
use crate::doctor;
use crate::key::Key;
use crate::mcp;
use crate::native::daemon;
use crate::output;
use crate::protocol::{
    Baseline, BatchStep, Call, ClickTarget, Direction, Expect, Get, Input, MouseButton, MouseEvent, Reply, Request,
    Response, RowRange, Scrollback, SessionInfo, Sessions, Signal, Size, SnapshotOptions, Style, TextMatch, Wait,
};
use crate::session_id;
use crate::skills;
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
    Snapshot(SnapshotArgs),
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
    #[command(about = "Save the tab as an asciicast (.cast) file that plays in asciinema")]
    Record {
        #[command(subcommand)]
        action: RecordAction,
    },
    #[command(about = "Everything about this session in one file, for a bug report")]
    Trace {
        #[command(subcommand)]
        action: TraceAction,
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
    #[command(
        about = "Mouse click on a ref, a cell or text (refused if the app has no mouse support; use keys instead)"
    )]
    Click(ClickArgs),
    #[command(about = "Scroll: the mouse wheel when the app tracks the mouse, else PageUp/PageDown")]
    Scroll(ScrollArgs),
    #[command(about = "Low-level mouse events at a cell: move, down, up, wheel")]
    Mouse {
        #[command(subcommand)]
        event: MouseCommand,
    },
    #[command(about = "Wait until the text is on screen, the screen is stable, or the program exits")]
    Wait(UntilArgs),
    #[command(about = "Fail now (exit 2) unless the screen or the program is as expected")]
    Expect(ExpectArgs),
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
    #[command(
        about = "Several commands in one call (args or stdin)",
        long_about = "Run several commands in order in one call to the daemon.\n\nPass each command as one quoted argument, e.g. batch \"open htop\" \"snapshot\", or pipe a JSON array to stdin whose items are command lines or argument arrays, e.g. [[\"open\", \"htop\"], \"snapshot\"]. Commands use the batch's --session, --tab and --timeout unless they set their own. Every command is parsed before any of them runs. Each command's output and exit code are printed in order, and batch exits with the code of the first command that failed."
    )]
    Batch(BatchArgs),
    #[command(about = "MCP stdio server with the same commands as tools")]
    Mcp,
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

    #[command(flatten)]
    wait: WaitArgs,

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
struct WaitArgs {
    #[arg(long, help = "Wait until this text is on screen, not just until the screen is stable", value_name = "TEXT")]
    wait: Option<String>,

    #[arg(
        long,
        help = "How long the screen must stay unchanged to count as stable",
        value_name = "MS",
        num_args = 0..=1,
        default_value_t = 300,
        default_missing_value = "300",
        conflicts_with = "wait"
    )]
    wait_stable: u64,
}

impl WaitArgs {
    fn until(self) -> Wait {
        match self.wait {
            Some(text) => Wait::Text(TextMatch::plain(text)),
            None => Wait::Stable { quiet_ms: self.wait_stable },
        }
    }
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

    #[arg(
        short,
        long,
        help = "Also list @t refs for what is on screen: panels, selected rows, buttons, links, key hints, the input under the cursor"
    )]
    interactive: bool,
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
            refs: self.interactive,
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

#[derive(Subcommand)]
enum RecordAction {
    #[command(
        about = "Save the tab to a .cast file from now on, starting from the screen as it is (default: ~/.agent-terminal/recordings/<session>-<tab>-<frame>.cast)"
    )]
    Start {
        #[arg(help = "Where to save it", value_name = "PATH")]
        path: Option<PathBuf>,
    },
    #[command(about = "Stop saving and print the file, its length and its size")]
    Stop,
}

#[derive(Subcommand)]
enum TraceAction {
    #[command(about = "Zip the session folder (activity log of every call, tab recordings) into one file")]
    Export {
        #[arg(help = "Where to write the zip file", value_name = "PATH")]
        path: PathBuf,
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
struct ClickArgs {
    #[arg(
        help = "A ref from snapshot -i (@t3), a cell (ROW,COL, from 1,1 at the top left) or text (its first match)",
        value_name = "@REF|ROW,COL|TEXT",
        value_parser = clap::value_parser!(ClickTarget),
        allow_hyphen_values = true
    )]
    target: ClickTarget,

    #[arg(long, help = "Click with the right button")]
    right: bool,

    #[arg(long, help = "Click twice, as a double click")]
    double: bool,

    #[command(flatten)]
    after: AfterInput,
}

#[derive(Args)]
struct ScrollArgs {
    #[arg(help = "up or down", value_name = "up|down", value_parser = clap::value_parser!(Direction))]
    direction: Direction,

    #[arg(
        help = "How many wheel notches or pages",
        value_name = "N",
        default_value_t = 1,
        value_parser = clap::value_parser!(u32).range(1..)
    )]
    count: u32,

    #[arg(long, help = "Press PageUp/PageDown even when the app tracks the mouse")]
    keys: bool,

    #[command(flatten)]
    after: AfterInput,
}

#[derive(Subcommand)]
enum MouseCommand {
    #[command(about = "Move the mouse to a cell (the app sees it only when it tracks motion, or drags)")]
    Move {
        #[arg(help = "Row,column, from 1,1 at the top left", value_name = "ROW,COL", value_parser = parse_position)]
        at: (u16, u16),
        #[command(flatten)]
        after: AfterInput,
    },
    #[command(about = "Press a mouse button at a cell")]
    Down {
        #[arg(help = "Row,column, from 1,1 at the top left", value_name = "ROW,COL", value_parser = parse_position)]
        at: (u16, u16),
        #[command(flatten)]
        button: ButtonArgs,
        #[command(flatten)]
        after: AfterInput,
    },
    #[command(about = "Release a mouse button at a cell")]
    Up {
        #[arg(help = "Row,column, from 1,1 at the top left", value_name = "ROW,COL", value_parser = parse_position)]
        at: (u16, u16),
        #[command(flatten)]
        button: ButtonArgs,
        #[command(flatten)]
        after: AfterInput,
    },
    #[command(about = "Turn the mouse wheel one notch at a cell")]
    Wheel {
        #[arg(help = "up or down", value_name = "up|down", value_parser = clap::value_parser!(Direction))]
        direction: Direction,
        #[arg(help = "Row,column, from 1,1 at the top left", value_name = "ROW,COL", value_parser = parse_position)]
        at: (u16, u16),
        #[command(flatten)]
        after: AfterInput,
    },
}

impl MouseCommand {
    fn event(self) -> (MouseEvent, AfterInput) {
        match self {
            MouseCommand::Move { at: (row, col), after } => (MouseEvent::Move { row, col }, after),
            MouseCommand::Down { at: (row, col), button, after } => {
                (MouseEvent::Down { row, col, button: button.button() }, after)
            }
            MouseCommand::Up { at: (row, col), button, after } => {
                (MouseEvent::Up { row, col, button: button.button() }, after)
            }
            MouseCommand::Wheel { direction, at: (row, col), after } => {
                (MouseEvent::Wheel { row, col, direction }, after)
            }
        }
    }
}

#[derive(Args)]
#[command(group(ArgGroup::new("which").args(["right", "middle"])))]
struct ButtonArgs {
    #[arg(long, help = "The right button instead of the left one")]
    right: bool,

    #[arg(long, help = "The middle button instead of the left one")]
    middle: bool,
}

impl ButtonArgs {
    fn button(self) -> MouseButton {
        match (self.right, self.middle) {
            (true, _) => MouseButton::Right,
            (_, true) => MouseButton::Middle,
            _ => MouseButton::Left,
        }
    }
}

#[derive(Args)]
#[command(group(ArgGroup::new("until").required(true).args(["text", "stable", "idle", "exit", "cursor", "frame"])))]
struct UntilArgs {
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

impl UntilArgs {
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
    let json = cli.json;
    let idle_timeout = cli.idle_timeout;
    let request = match cli.command {
        Command::Daemon { action: DaemonAction::Run } => {
            return match daemon::run(idle_timeout) {
                Ok(()) => 0,
                Err(e) => output::fail(&format!("{e:#}"), crate::exit::ERROR),
            };
        }
        Command::Skills { action } => return skills::run(action, json, Cli::command()),
        Command::Install { .. } => return skills::install(json),
        Command::Mcp => return mcp::run(Cli::command()),
        Command::Session { action: None } => {
            output::session(&cli.session, json);
            return 0;
        }
        Command::Session { action: Some(SessionAction::Id { scope: Scope::Worktree, prefix }) } => {
            let id = std::env::current_dir()
                .map_err(anyhow::Error::from)
                .and_then(|cwd| session_id::for_worktree(&cwd, prefix.as_deref()));
            return match id {
                Ok(id) => {
                    output::session_id(&id, json);
                    0
                }
                Err(e) => output::fail(&format!("{e:#}"), crate::exit::ERROR),
            };
        }
        Command::Version => return output::version(json),
        Command::Doctor { fix } => return doctor::run(fix, json),
        Command::Upgrade => return upgrade::run(json),
        Command::Batch(args) => batch(&args, &cli.session, cli.tab, cli.timeout),
        command => request(Cli { command, ..cli }),
    };
    match request {
        Ok(request) => send(request, json, idle_timeout),
        Err(message) => output::fail(&message, crate::exit::ERROR),
    }
}

fn request(cli: Cli) -> Result<Request, String> {
    let session = cli.session;
    let request = match cli.command {
        Command::Daemon { action: DaemonAction::Stop } => Request::Stop,
        Command::Open(args) | Command::Tab { action: Some(TabAction::New(args)), .. } => {
            open(session, args, cli.timeout)?
        }
        Command::Tab { id: Some(tab), .. } => Request::SwitchTab { session, tab },
        Command::Tab { id: None, action: None | Some(TabAction::List) } => Request::Tabs { session },
        Command::Tab { action: Some(TabAction::Close { id }), .. } => {
            Request::Close { session, all: false, tab: id.or(cli.tab) }
        }
        Command::Session { action: Some(SessionAction::List) } => Request::Sessions,
        Command::Restart(wait) => {
            Request::Restart { session, tab: cli.tab, wait: wait.until(), timeout_ms: cli.timeout }
        }
        Command::Resize { size, wait } => {
            Request::Resize { session, tab: cli.tab, size, wait: wait.until(), timeout_ms: cli.timeout }
        }
        Command::Snapshot(args) => Request::Snapshot { session, tab: cli.tab, options: args.options() },
        Command::Close { all } => Request::Close { session, all, tab: cli.tab },
        Command::Get { what } => Request::Get { session, tab: cli.tab, what: what.request() },
        Command::Find { text, regex, all } => Request::Find { session, tab: cli.tab, text, regex, all },
        Command::Mark { name } => Request::Mark { session, tab: cli.tab, name },
        Command::Output { since, raw } => Request::Output { session, tab: cli.tab, since, raw },
        Command::Diff { what: DiffCommand::Snapshot { baseline } } => {
            let baseline = baseline.map(|path| read_baseline(&path)).transpose()?;
            Request::DiffSnapshot { session, tab: cli.tab, baseline }
        }
        Command::Record { action: RecordAction::Start { path } } => {
            let path = path.map(|path| absolute(&path)).transpose()?;
            Request::RecordStart { session, tab: cli.tab, path }
        }
        Command::Record { action: RecordAction::Stop } => Request::RecordStop { session, tab: cli.tab },
        Command::Trace { action: TraceAction::Export { path } } => {
            Request::TraceExport { session, path: absolute(&path)? }
        }
        Command::Type(args) => {
            input(session, cli.tab, Input::Type { text: args.text, delay_ms: args.delay }, args.after, cli.timeout)
        }
        Command::Press(args) => {
            input(session, cli.tab, Input::Press { keys: args.keys, repeat: args.repeat }, args.after, cli.timeout)
        }
        Command::Paste(args) => input(session, cli.tab, Input::Paste { text: args.text }, args.after, cli.timeout),
        Command::Send(args) => input(session, cli.tab, Input::Send { bytes: args.bytes.0 }, args.after, cli.timeout),
        Command::Signal(args) => {
            input(session, cli.tab, Input::Signal { signal: args.signal }, args.after, cli.timeout)
        }
        Command::Click(args) => {
            let button = if args.right { MouseButton::Right } else { MouseButton::Left };
            let click = Input::Click { target: args.target, button, double: args.double };
            input(session, cli.tab, click, args.after, cli.timeout)
        }
        Command::Scroll(args) => {
            let scroll = Input::Scroll { direction: args.direction, count: args.count, keys: args.keys };
            input(session, cli.tab, scroll, args.after, cli.timeout)
        }
        Command::Mouse { event } => {
            let (event, after) = event.event();
            input(session, cli.tab, Input::Mouse { event }, after, cli.timeout)
        }
        Command::Wait(args) => Request::Wait { session, tab: cli.tab, wait: args.until(), timeout_ms: cli.timeout },
        Command::Expect(args) => Request::Expect { session, tab: cli.tab, expect: args.check() },
        Command::Daemon { action: DaemonAction::Run }
        | Command::Skills { .. }
        | Command::Install { .. }
        | Command::Session { action: None | Some(SessionAction::Id { .. }) }
        | Command::Batch(_)
        | Command::Mcp
        | Command::Doctor { .. }
        | Command::Upgrade
        | Command::Version => {
            return Err("this command runs on its own, not inside batch".into());
        }
    };

    Ok(request)
}

fn batch(args: &BatchArgs, session: &str, tab: Option<u32>, timeout: u64) -> Result<Request, String> {
    let parser = Cli::command()
        .no_binary_name(true)
        .mut_arg("session", |arg| arg.env(None::<&str>).default_value(session.to_string()))
        .mut_arg("tab", |arg| match tab {
            Some(id) => arg.default_value(id.to_string()),
            None => arg,
        })
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

fn send(request: Request, json: bool, idle_timeout: Duration) -> i32 {
    let if_not_running = match &request {
        Request::Stop => IfNotRunning::Answer(Reply::Stopped),
        Request::Sessions => IfNotRunning::Answer(Reply::Sessions(Sessions::default())),
        Request::Tabs { session } => IfNotRunning::Answer(Reply::Tabs(SessionInfo::empty(session))),
        _ => IfNotRunning::Start(Launch { idle_timeout }),
    };
    let typed = std::env::args_os().skip(1).map(|arg| arg.to_string_lossy().into_owned()).collect();
    let response = match connection::send(&Call { typed: Some(typed), request }, if_not_running) {
        Ok(response) => response,
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

fn input(session: String, tab: Option<u32>, input: Input, after: AfterInput, timeout_ms: u64) -> Request {
    let snapshot = after.snapshot;
    Request::Input { session, tab, input, wait: after.wait(), snapshot, timeout_ms }
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
        wait: (!args.no_wait).then(|| args.wait.until()),
        timeout_ms,
    })
}

fn absolute(path: &std::path::Path) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|e| format!("current directory: {e}"))
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
