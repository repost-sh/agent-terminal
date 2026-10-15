use clap::{ArgGroup, Args};

use super::Context;
use super::values::{parse_position, parse_rows};
use crate::protocol::request;
use crate::protocol::{Expect, Request, RowRange, Style, TextMatch, Wait};

#[derive(Args)]
#[command(group(ArgGroup::new("until").required(true).args(["text", "stable", "idle", "exit", "cursor", "frame", "command", "control"])))]
pub(super) struct UntilArgs {
    #[command(flatten)]
    text: TextUntil,

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

    #[command(flatten)]
    exit: ExitUntil,

    #[arg(long, help = "Until the cursor is at this row and column", value_name = "ROW,COL", value_parser = parse_position)]
    cursor: Option<(u16, u16)>,

    #[arg(long, help = "Until the screen reaches this frame number", value_name = "N")]
    frame: Option<u64>,

    #[arg(long, help = "Until the shell command finishes and the shell shows its prompt (shell tabs)")]
    command: bool,

    #[arg(long, help = "Until a human hands control back (at once when the agent has it)")]
    control: bool,

    #[arg(long, help = "Treat running out of time as a failed check: exit 2 instead of 3")]
    assert: bool,
}

#[derive(Args)]
struct TextUntil {
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
}

#[derive(Args)]
struct ExitUntil {
    #[arg(long, help = "Until the program exits")]
    exit: bool,

    #[arg(long, requires = "exit", help = "Fail (exit 2) unless the program exits with this code", value_name = "N")]
    code: Option<i32>,
}

impl UntilArgs {
    pub(super) fn request(self, context: Context) -> Result<Request, String> {
        let Context { target, timeout, .. } = context;
        let assert = self.assert;
        if self.control {
            return Ok(Request::WaitControl(request::WaitControl { target, timeout_ms: timeout, assert }));
        }
        Ok(Request::Wait(request::WaitFor { target, wait: self.until()?, timeout_ms: timeout, assert }))
    }

    fn until(self) -> Result<Wait, String> {
        match self.chosen() {
            Some(wait) => Ok(wait),
            None => self.text.until(),
        }
    }

    fn chosen(&self) -> Option<Wait> {
        self.stable
            .map(|quiet_ms| Wait::Stable { quiet_ms })
            .or(self.idle.map(|quiet_ms| Wait::Idle { quiet_ms }))
            .or(self.exit.exit.then_some(Wait::Exit { code: self.exit.code }))
            .or(self.cursor.map(|(row, col)| Wait::Cursor { row, col }))
            .or(self.frame.map(|frame| Wait::Frame { frame }))
            .or(self.command.then_some(Wait::Command))
    }
}

impl TextUntil {
    fn until(self) -> Result<Wait, String> {
        let text = self
            .text
            .ok_or("wait needs text, a number of milliseconds, --stable, --idle, --exit, --cursor, --frame, --command or --control")?;
        let plain = !self.regex && !self.gone && self.rows.is_none();
        match text.parse() {
            Ok(ms) if plain => Ok(Wait::Elapsed { ms }),
            _ => {
                let text = TextMatch { text, regex: self.regex, rows: self.rows };
                Ok(if self.gone { Wait::Gone(text) } else { Wait::Text(text) })
            }
        }
    }
}

#[derive(Args)]
#[command(group(ArgGroup::new("check").required(true).args(["text", "exit_code", "cell"])))]
pub(super) struct ExpectArgs {
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
    pub(super) fn check(self) -> Result<Expect, String> {
        if let Some(code) = self.exit_code {
            return Ok(Expect::ExitCode { code });
        }
        if let Some((row, col)) = self.cell {
            return Ok(Expect::Cell { row, col, style: self.style });
        }
        let text = TextMatch::plain(self.text.ok_or("expect needs text, --exit-code or --cell")?);
        Ok(if self.gone { Expect::Gone(text) } else { Expect::Text(text) })
    }
}
