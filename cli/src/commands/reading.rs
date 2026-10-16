use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use super::Context;
use super::values::{parse_rows, parse_scrollback, parse_threshold};
use crate::output;
use crate::protocol::request;
use crate::protocol::{
    Baseline, Get, Look, Request, RowRange, ScreenshotFormat, Scrollback, Shot, SnapshotOptions, Theme,
};

#[derive(Subcommand)]
pub(super) enum ReadCommand {
    #[command(about = "One fact about the terminal, e.g. get text, get cursor, get cell 4 3")]
    Get {
        #[command(subcommand)]
        what: GetCommand,
    },
    #[command(about = "Save the screen as a PNG drawn with bundled fonts (or SVG, HTML, text or ANSI)")]
    Screenshot(ScreenshotArgs),
    #[command(about = "Where the text is on screen (row,col)")]
    Find {
        #[arg(help = "Text to look for")]
        text: String,
        #[arg(long, help = "Treat the text as a regular expression")]
        regex: bool,
        #[arg(long, help = "Also look in the scrollback above the screen (negative rows)")]
        all: bool,
    },
    #[command(about = "Name this point in the output, to read from later")]
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
    #[command(about = "Everything about this session in one file, for a bug report")]
    Trace {
        #[command(subcommand)]
        action: TraceAction,
    },
}

impl ReadCommand {
    pub(super) fn request(self, context: Context) -> Result<Request, String> {
        let Context { target, dir, .. } = context;
        Ok(match self {
            ReadCommand::Get { what } => Request::Get(request::Get { target, what: what.request() }),
            ReadCommand::Find { text, regex, all } => Request::Find(request::Find { target, text, regex, all }),
            ReadCommand::Mark { name } => Request::Mark(request::Mark { target, name }),
            ReadCommand::Output { since, raw } => Request::Output(request::Output { target, since, raw }),
            ReadCommand::Diff { what: DiffCommand::Snapshot { baseline } } => {
                let baseline = baseline.map(|path| read_baseline(&path)).transpose()?;
                Request::DiffSnapshot(request::DiffSnapshot { target, baseline })
            }
            ReadCommand::Diff { what: DiffCommand::Screenshot { baseline, output, threshold, look } } => {
                let png = ScreenshotArgs { path: output, annotate: false, format: Some(ScreenshotFormat::Png), look };
                Request::DiffScreenshot(request::DiffScreenshot {
                    target,
                    baseline: absolute(&baseline)?,
                    shot: png.shot(dir)?,
                    threshold,
                })
            }
            ReadCommand::Trace { action: TraceAction::Export { path } } => {
                Request::TraceExport(request::TraceExport { session: target.session, path: absolute(&path)? })
            }
            ReadCommand::Screenshot(args) => Request::Screenshot(request::Screenshot { target, shot: args.shot(dir)? }),
        })
    }
}

#[derive(Args)]
#[group(id = "view")]
pub(super) struct SnapshotArgs {
    #[arg(long, help = "Only rows A to B, counted from 1 at the top", value_name = "A:B", value_parser = parse_rows)]
    rows: Option<RowRange>,

    #[arg(
        long,
        help = "Add N lines of scrollback above the screen (all of it when N is left out)",
        value_name = "N",
        num_args = 0..=1,
        default_missing_value = "",
        value_parser = parse_scrollback
    )]
    scrollback: Option<Scrollback>,

    #[arg(long, help = "Keep colors and styles as ANSI escape codes")]
    ansi: bool,

    #[arg(
        short,
        long,
        help = "Also list @t refs for what is on screen: panels, selected rows, buttons, links, key hints, the input under the cursor"
    )]
    interactive: bool,

    #[arg(short, long, help = "Fold each run of 3 or more blank rows into one line")]
    squeeze_blank: bool,
}

impl SnapshotArgs {
    pub(super) fn options(self) -> SnapshotOptions {
        SnapshotOptions {
            rows: self.rows,
            scrollback: self.scrollback.unwrap_or_default(),
            ansi: self.ansi,
            refs: self.interactive,
            squeeze_blank: self.squeeze_blank,
        }
    }
}

#[derive(Args)]
pub(super) struct ScreenshotArgs {
    #[arg(help = "Where to save it (default: <screenshot dir>/<session>-<tab>-<frame>.<format>)", value_name = "PATH")]
    pub(super) path: Option<PathBuf>,

    #[arg(long, help = "Draw a box around each @t ref with its name, and a legend of the refs under the screen")]
    pub(super) annotate: bool,

    #[arg(
        long,
        help = "png, svg, html, txt or ansi (default: from the file's extension, else png)",
        value_name = "FORMAT",
        value_parser = clap::value_parser!(ScreenshotFormat)
    )]
    pub(super) format: Option<ScreenshotFormat>,

    #[command(flatten)]
    pub(super) look: LookArgs,
}

impl ScreenshotArgs {
    pub(super) fn shot(self, dir: Option<PathBuf>) -> Result<Shot, String> {
        let format = self.format.or_else(|| format_of(self.path.as_deref())).unwrap_or_default();
        if self.annotate && !matches!(format, ScreenshotFormat::Png | ScreenshotFormat::Svg) {
            return Err(format!("--annotate draws on an image, so it needs png or svg, not {format}"));
        }
        Ok(Shot {
            path: self.path.map(|path| absolute(&path)).transpose()?,
            dir: dir.map(|dir| absolute(&dir)).transpose()?,
            format,
            look: self.look.look(),
            rows: self.look.rows,
            annotate: self.annotate,
        })
    }
}

fn format_of(path: Option<&Path>) -> Option<ScreenshotFormat> {
    let extension = path?.extension()?.to_str()?.to_ascii_lowercase();
    ScreenshotFormat::ALL.into_iter().find(|format| format.name() == extension)
}

#[derive(Args)]
pub(super) struct LookArgs {
    #[arg(
        long,
        help = "Pixels per cell: 2 for a sharper image, twice as wide and high",
        value_name = "1|2",
        default_value_t = 1,
        value_parser = clap::value_parser!(u8).range(1..=2)
    )]
    scale: u8,

    #[arg(
        long,
        help = "Colors: dark or light",
        value_name = "THEME",
        default_value = "dark",
        value_parser = clap::value_parser!(Theme)
    )]
    theme: Theme,

    #[arg(long, help = "Only rows A to B, counted from 1 at the top", value_name = "A:B", value_parser = parse_rows)]
    rows: Option<RowRange>,

    #[arg(long, help = "Draw the cursor where the program shows it (the default)", overrides_with = "no_cursor")]
    cursor: bool,

    #[arg(long, help = "Leave the cursor out", overrides_with = "cursor")]
    no_cursor: bool,
}

impl LookArgs {
    pub(super) fn plain() -> Self {
        Self { scale: 1, theme: Theme::Dark, rows: None, cursor: true, no_cursor: false }
    }

    fn look(&self) -> Look {
        Look { scale: self.scale, theme: self.theme, cursor: !self.no_cursor }
    }
}

#[derive(Subcommand)]
pub(super) enum GetCommand {
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
    #[command(about = "Title the program set for its terminal (OSC 2)")]
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
pub(super) enum DiffCommand {
    #[command(about = "Rows that changed since the last snapshot (or since a saved one)")]
    Snapshot {
        #[arg(long, help = "Compare with a snapshot saved to this file instead", value_name = "FILE")]
        baseline: Option<PathBuf>,
    },
    #[command(
        about = "Pixels that differ from a baseline PNG, drawn into a diff image; exits 2 when more differ than --threshold"
    )]
    Screenshot {
        #[arg(long, help = "The PNG to compare with, e.g. an earlier `agent-terminal screenshot`", value_name = "PNG")]
        baseline: PathBuf,
        #[arg(
            long,
            help = "Where to save the diff image (default: <screenshot dir>/<session>-<tab>-<frame>-diff.png)",
            value_name = "PATH"
        )]
        output: Option<PathBuf>,
        #[arg(
            long,
            help = "Share of pixels that may differ and still pass, from 0 to 1",
            value_name = "0-1",
            default_value_t = 0.0,
            value_parser = parse_threshold
        )]
        threshold: f64,
        #[command(flatten)]
        look: LookArgs,
    },
}

#[derive(Subcommand)]
pub(super) enum TraceAction {
    #[command(about = "Zip the session folder (activity log, screenshot list, command logs) with its screenshots")]
    Export {
        #[arg(help = "Where to write the zip file", value_name = "PATH")]
        path: PathBuf,
    },
}

fn absolute(path: &Path) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|e| format!("current directory: {e}"))
}

fn read_baseline(path: &Path) -> Result<Baseline, String> {
    let name = path.display().to_string();
    let saved = std::fs::read_to_string(path).map_err(|e| format!("read the baseline {name}: {e}"))?;
    let lines = output::saved_rows(&saved)
        .ok_or_else(|| format!("{name} is not a snapshot; save one with `agent-terminal snapshot > {name}`"))?;
    Ok(Baseline { name, lines })
}
