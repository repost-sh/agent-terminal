use clap::{ArgGroup, Args, Subcommand};

use super::Context;
use super::reading::{LookArgs, ScreenshotArgs};
use super::values::{RawBytes, ScreenshotPath, parse_bytes, parse_position};
use crate::key::Key;
use crate::protocol::request;
use crate::protocol::{ClickTarget, Direction, Input, MouseButton, MouseEvent, Request, Signal, TextMatch, Wait};

#[derive(Subcommand)]
pub(super) enum InputCommand {
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
}

impl InputCommand {
    pub(super) fn request(self, context: Context) -> Result<Request, String> {
        let (input, after) = self.input();
        let wait = after.wait();
        let screenshot = after
            .screenshot
            .map(|path| {
                let path = path.chosen();
                ScreenshotArgs { path, annotate: false, format: None, look: LookArgs::plain() }.shot(context.dir)
            })
            .transpose()?;
        Ok(Request::Input(request::Deliver {
            target: context.target,
            input,
            wait,
            snapshot: after.snapshot,
            screenshot,
            timeout_ms: context.timeout,
        }))
    }

    fn input(self) -> (Input, AfterInput) {
        match self {
            InputCommand::Type(args) => (Input::Type { text: args.text, delay_ms: args.delay }, args.after),
            InputCommand::Press(args) => (Input::Press { keys: args.keys, repeat: args.repeat }, args.after),
            InputCommand::Paste(args) => (Input::Paste { text: args.text }, args.after),
            InputCommand::Send(args) => (Input::Send { bytes: args.bytes.0 }, args.after),
            InputCommand::Signal(args) => (Input::Signal { signal: args.signal }, args.after),
            InputCommand::Click(args) => {
                let button = if args.right { MouseButton::Right } else { MouseButton::Left };
                (Input::Click { target: args.target, button, double: args.double }, args.after)
            }
            InputCommand::Scroll(args) => {
                (Input::Scroll { direction: args.direction, count: args.count, keys: args.keys }, args.after)
            }
            InputCommand::Mouse { event } => {
                let (event, after) = event.event();
                (Input::Mouse { event }, after)
            }
        }
    }
}

#[derive(Args)]
pub(super) struct TypeArgs {
    #[arg(help = "The text; a newline presses Enter, a tab presses Tab", allow_hyphen_values = true)]
    text: String,

    #[arg(long, help = "Pause this long between keys", value_name = "MS", default_value_t = 0)]
    delay: u64,

    #[command(flatten)]
    after: AfterInput,
}

#[derive(Args)]
pub(super) struct PressArgs {
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
pub(super) struct PasteArgs {
    #[arg(help = "The text, pasted in one piece", allow_hyphen_values = true)]
    text: String,

    #[command(flatten)]
    after: AfterInput,
}

#[derive(Args)]
pub(super) struct SendArgs {
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
pub(super) struct SignalArgs {
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
pub(super) struct ClickArgs {
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
pub(super) struct ScrollArgs {
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
pub(super) enum MouseCommand {
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
pub(super) struct ButtonArgs {
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
#[command(group(ArgGroup::new("then").args(["wait", "wait_gone", "wait_stable"])))]
pub(super) struct AfterInput {
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

    #[arg(
        long,
        help = "Then save a PNG of the screen (default: <screenshot dir>/<session>-<tab>-<frame>.png)",
        value_name = "PATH",
        num_args = 0..=1,
        default_missing_value = "",
        value_parser = clap::value_parser!(ScreenshotPath)
    )]
    screenshot: Option<ScreenshotPath>,
}

impl AfterInput {
    fn wait(&self) -> Option<Wait> {
        match (self.wait.clone(), self.wait_gone.clone(), self.wait_stable) {
            (Some(text), _, _) => Some(Wait::Text(TextMatch::plain(text))),
            (_, Some(text), _) => Some(Wait::Gone(TextMatch::plain(text))),
            (_, _, Some(quiet_ms)) => Some(Wait::Stable { quiet_ms }),
            (None, None, None) => None,
        }
    }
}
