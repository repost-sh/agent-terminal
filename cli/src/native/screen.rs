use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;
use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Row};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell as GridCell, Flags};
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor};
use alacritty_terminal::vte::{Parser, Perform};

use crate::native::refs::{Glyph, Grid, Link};
use crate::protocol::{Cell, Modes, Mouse, Size};

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }

    fn screen_lines(&self) -> usize {
        self.rows as usize
    }

    fn columns(&self) -> usize {
        self.cols as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseMode {
    X10,
    Sgr,
    Utf8,
}

impl MouseMode {
    pub fn as_str(self) -> &'static str {
        match self {
            MouseMode::X10 => "x10",
            MouseMode::Sgr => "sgr",
            MouseMode::Utf8 => "utf8",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseTracking {
    Click,
    Drag,
    Motion,
}

impl MouseTracking {
    fn as_str(self) -> &'static str {
        match self {
            MouseTracking::Click => "click",
            MouseTracking::Drag => "drag",
            MouseTracking::Motion => "motion",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseReporting {
    pub tracking: MouseTracking,
    pub encoding: MouseMode,
}

#[derive(Clone, Debug)]
pub struct ScreenView {
    pub size: Size,
    pub lines: Vec<String>,
    pub cursor: (u16, u16),
    pub cursor_visible: bool,
    pub alt_screen: bool,
    pub mouse: Option<MouseMode>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyboardModes {
    pub app_cursor: bool,
    pub bracketed_paste: bool,
    pub kitty_flags: u8,
}

const KITTY_FLAGS: [(TermMode, u8, &str); 5] = [
    (TermMode::DISAMBIGUATE_ESC_CODES, 1, "disambiguate"),
    (TermMode::REPORT_EVENT_TYPES, 2, "report-events"),
    (TermMode::REPORT_ALTERNATE_KEYS, 4, "report-alternates"),
    (TermMode::REPORT_ALL_KEYS_AS_ESC, 8, "report-all-keys"),
    (TermMode::REPORT_ASSOCIATED_TEXT, 16, "report-text"),
];

fn is_spacer(cell: &GridCell) -> bool {
    cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
}

fn push_cell(text: &mut String, cell: &GridCell) {
    text.push(cell.c);
    if let Some(zerowidth) = cell.zerowidth() {
        text.extend(zerowidth);
    }
}

fn plain_row(row: &Row<GridCell>, cols: usize) -> String {
    located_row(row, cols).0
}

fn located_row(row: &Row<GridCell>, cols: usize) -> (String, Vec<u16>) {
    let mut text = String::with_capacity(cols);
    let mut columns = Vec::with_capacity(cols);
    for col in (0..cols).filter(|&col| !is_spacer(&row[Column(col)])) {
        push_cell(&mut text, &row[Column(col)]);
        columns.resize(text.len(), col as u16);
    }
    text.truncate(text.trim_end().len());
    columns.truncate(text.len());
    (text, columns)
}

fn styled_row(row: &Row<GridCell>, cols: usize) -> String {
    let shows = |cell: &GridCell| {
        cell.c != ' '
            || cell.bg != Color::Named(NamedColor::Background)
            || cell.flags.intersects(Flags::INVERSE | Flags::ALL_UNDERLINES | Flags::STRIKEOUT)
    };
    let end = (0..cols).rposition(|col| shows(&row[Column(col)])).map_or(0, |i| i + 1);
    let mut text = String::new();
    let mut current = String::new();
    for cell in (0..end).map(|col| &row[Column(col)]).filter(|cell| !is_spacer(cell)) {
        let style = sgr(cell);
        if style != current {
            if style.is_empty() {
                text.push_str("\x1b[0m");
            } else {
                text.push_str(&format!("\x1b[0;{style}m"));
            }
            current = style;
        }
        push_cell(&mut text, cell);
    }
    if !current.is_empty() {
        text.push_str("\x1b[0m");
    }
    text
}

fn sgr(cell: &GridCell) -> String {
    let attributes = [
        (Flags::BOLD, "1"),
        (Flags::DIM, "2"),
        (Flags::ITALIC, "3"),
        (Flags::ALL_UNDERLINES, "4"),
        (Flags::INVERSE, "7"),
        (Flags::HIDDEN, "8"),
        (Flags::STRIKEOUT, "9"),
    ];
    let mut params: Vec<String> =
        attributes.iter().filter(|(flag, _)| cell.flags.intersects(*flag)).map(|(_, code)| code.to_string()).collect();
    params.extend(color_sgr(cell.fg, 30));
    params.extend(color_sgr(cell.bg, 40));
    params.join(";")
}

fn color_sgr(color: Color, base: u16) -> Option<String> {
    match color {
        Color::Spec(rgb) => Some(format!("{};2;{};{};{}", base + 8, rgb.r, rgb.g, rgb.b)),
        Color::Indexed(index) => Some(format!("{};5;{index}", base + 8)),
        Color::Named(named) => {
            let index = named as u16;
            let dim_black = NamedColor::DimBlack as u16;
            match index {
                0..=7 => Some((base + index).to_string()),
                8..=15 => Some((base + 60 + index - 8).to_string()),
                _ if (dim_black..dim_black + 8).contains(&index) => Some((base + index - dim_black).to_string()),
                _ => None,
            }
        }
    }
}

const COLOR_NAMES: [&str; 16] = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright-black",
    "bright-red",
    "bright-green",
    "bright-yellow",
    "bright-blue",
    "bright-magenta",
    "bright-cyan",
    "bright-white",
];

fn color_name(color: Color) -> String {
    match color {
        Color::Spec(rgb) => format!("#{:02x}{:02x}{:02x}", rgb.r, rgb.g, rgb.b),
        Color::Indexed(index) => COLOR_NAMES.get(index as usize).map_or_else(|| index.to_string(), |n| n.to_string()),
        Color::Named(named) => match named {
            NamedColor::DimBlack => "black",
            NamedColor::DimRed => "red",
            NamedColor::DimGreen => "green",
            NamedColor::DimYellow => "yellow",
            NamedColor::DimBlue => "blue",
            NamedColor::DimMagenta => "magenta",
            NamedColor::DimCyan => "cyan",
            NamedColor::DimWhite => "white",
            other => COLOR_NAMES.get(other as usize).copied().unwrap_or("default"),
        }
        .to_string(),
    }
}

fn glyph(cell: &GridCell) -> Glyph {
    let mut text = String::new();
    if !is_spacer(cell) {
        push_cell(&mut text, cell);
    }
    Glyph {
        text,
        fg: color_name(cell.fg),
        bg: color_name(cell.bg),
        inverse: cell.flags.contains(Flags::INVERSE),
        bold: cell.flags.contains(Flags::BOLD),
        underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
        link: cell.hyperlink().map(|link| Link { id: link.id().to_string(), url: link.uri().to_string() }),
    }
}

fn hash_color(color: Color, hasher: &mut DefaultHasher) {
    match color {
        Color::Named(named) => (0u8, named as u16).hash(hasher),
        Color::Spec(rgb) => (1u8, rgb.r, rgb.g, rgb.b).hash(hasher),
        Color::Indexed(index) => (2u8, index).hash(hasher),
    }
}

fn mouse_encoding(mode: TermMode) -> Option<MouseMode> {
    mode.intersects(TermMode::MOUSE_MODE).then(|| {
        if mode.contains(TermMode::SGR_MOUSE) {
            MouseMode::Sgr
        } else if mode.contains(TermMode::UTF8_MOUSE) {
            MouseMode::Utf8
        } else {
            MouseMode::X10
        }
    })
}

#[derive(Default)]
struct Reported {
    replies: Vec<u8>,
    title: Option<String>,
}

#[derive(Clone, Default)]
struct Events(Arc<Mutex<Reported>>);

impl EventListener for Events {
    fn send_event(&self, event: Event) {
        let mut reported = self.0.lock().unwrap();
        match event {
            Event::PtyWrite(text) => reported.replies.extend_from_slice(text.as_bytes()),
            Event::Title(title) => reported.title = Some(title),
            Event::ResetTitle => reported.title = None,
            _ => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellReport {
    Prompt,
    Ready,
    Started { command: String, cwd: Option<String> },
    Finished { exit_code: Option<i32> },
}

#[derive(Default)]
struct Stream {
    cwd: Option<String>,
    printed: String,
    reports: Vec<(usize, ShellReport)>,
}

impl Stream {
    fn report(&mut self, params: &[&[u8]]) {
        let report = match params {
            [b"A", ..] => ShellReport::Prompt,
            [b"B", ..] => ShellReport::Ready,
            [b"C", options @ ..] => ShellReport::Started {
                command: options.iter().find_map(|option| command_line(option)).unwrap_or_default(),
                cwd: self.cwd.clone(),
            },
            [b"D", code @ ..] => ShellReport::Finished {
                exit_code: code.first().and_then(|code| std::str::from_utf8(code).ok()?.parse().ok()),
            },
            _ => return,
        };
        self.reports.push((self.printed.len(), report));
    }
}

fn command_line(option: &[u8]) -> Option<String> {
    if let Some(encoded) = option.strip_prefix(b"cmdline_url=") {
        return Some(String::from_utf8_lossy(&percent_decoded(encoded)).into_owned());
    }
    option.strip_prefix(b"cmdline=").map(|text| String::from_utf8_lossy(text).into_owned())
}

impl Perform for Stream {
    fn print(&mut self, c: char) {
        self.printed.push(c);
    }

    fn execute(&mut self, byte: u8) {
        if matches!(byte, b'\n' | b'\t') {
            self.printed.push(byte as char);
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        match params {
            [b"7", url @ ..] => {
                if let Some(cwd) = file_url_path(&url.join(&b';')) {
                    self.cwd = Some(cwd);
                }
            }
            [b"133", mark @ ..] => self.report(mark),
            _ => {}
        }
    }
}

fn file_url_path(url: &[u8]) -> Option<String> {
    let rest = url.strip_prefix(b"file://")?;
    let path = &rest[rest.iter().position(|&b| b == b'/')?..];
    Some(String::from_utf8_lossy(&percent_decoded(path)).into_owned())
}

fn percent_decoded(path: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(path.len());
    let mut i = 0;
    while i < path.len() {
        let escaped = (path[i] == b'%')
            .then(|| path.get(i + 1..i + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        match escaped {
            Some(byte) => {
                decoded.push(byte);
                i += 3;
            }
            None => {
                decoded.push(path[i]);
                i += 1;
            }
        }
    }
    decoded
}

pub struct Screen {
    term: Term<Events>,
    parser: Processor,
    events: Events,
    stream_parser: Parser,
    stream: Stream,
    size: Size,
}

impl Screen {
    pub fn new(size: Size) -> Self {
        let events = Events::default();
        let config = Config { kitty_keyboard: true, scrolling_history: 2000, ..Config::default() };
        Self {
            term: Term::new(config, &size, events.clone()),
            parser: Processor::new(),
            events,
            stream_parser: Parser::new(),
            stream: Stream::default(),
            size,
        }
    }

    pub fn keyboard_modes(&self) -> KeyboardModes {
        let mode = *self.term.mode();
        KeyboardModes {
            app_cursor: mode.contains(TermMode::APP_CURSOR),
            bracketed_paste: mode.contains(TermMode::BRACKETED_PASTE),
            kitty_flags: KITTY_FLAGS.iter().filter(|(flag, ..)| mode.contains(*flag)).map(|(_, bit, _)| bit).sum(),
        }
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn resize(&mut self, size: Size) {
        self.term.resize(size);
        self.size = size;
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.parser.advance(&mut self.term, bytes);
        self.stream_parser.advance(&mut self.stream, bytes);
        std::mem::take(&mut self.events.0.lock().unwrap().replies)
    }

    pub fn scrollback_len(&self) -> usize {
        self.term.grid().history_size()
    }

    pub fn rows(&self, lines: Range<i32>, ansi: bool) -> Vec<String> {
        let grid = self.term.grid();
        let cols = self.size.cols as usize;
        lines
            .map(|line| if ansi { styled_row(&grid[Line(line)], cols) } else { plain_row(&grid[Line(line)], cols) })
            .collect()
    }

    pub fn located_row(&self, line: i32) -> (String, Vec<u16>) {
        located_row(&self.term.grid()[Line(line)], self.size.cols as usize)
    }

    pub fn mouse_reporting(&self) -> Option<MouseReporting> {
        let mode = *self.term.mode();
        let tracking = if mode.contains(TermMode::MOUSE_MOTION) {
            Some(MouseTracking::Motion)
        } else if mode.contains(TermMode::MOUSE_DRAG) {
            Some(MouseTracking::Drag)
        } else if mode.contains(TermMode::MOUSE_REPORT_CLICK) {
            Some(MouseTracking::Click)
        } else {
            None
        };
        tracking.zip(mouse_encoding(mode)).map(|(tracking, encoding)| MouseReporting { tracking, encoding })
    }

    pub fn modes(&self) -> Modes {
        let mode = *self.term.mode();
        Modes {
            alt_screen: mode.contains(TermMode::ALT_SCREEN),
            mouse: self.mouse_reporting().map(|reporting| Mouse {
                tracking: reporting.tracking.as_str().to_string(),
                encoding: reporting.encoding.as_str().to_string(),
            }),
            bracketed_paste: mode.contains(TermMode::BRACKETED_PASTE),
            app_cursor: mode.contains(TermMode::APP_CURSOR),
            focus_events: mode.contains(TermMode::FOCUS_IN_OUT),
            keyboard: KITTY_FLAGS
                .iter()
                .filter(|(flag, ..)| mode.contains(*flag))
                .map(|(.., name)| name.to_string())
                .collect(),
        }
    }

    pub fn cell(&self, row: u16, col: u16) -> Option<Cell> {
        if !(1..=self.size.rows).contains(&row) || !(1..=self.size.cols).contains(&col) {
            return None;
        }
        let cell = &self.term.grid()[Line(row as i32 - 1)][Column(col as usize - 1)];
        let mut char = cell.c.to_string();
        if let Some(zerowidth) = cell.zerowidth() {
            char.extend(zerowidth);
        }
        Some(Cell {
            row,
            col,
            char,
            fg: color_name(cell.fg),
            bg: color_name(cell.bg),
            bold: cell.flags.contains(Flags::BOLD),
            inverse: cell.flags.contains(Flags::INVERSE),
            underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
        })
    }

    pub fn fingerprint(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        let grid = self.term.grid();
        for line in 0..self.size.rows as i32 {
            let row = &grid[Line(line)];
            for col in 0..self.size.cols as usize {
                let cell = &row[Column(col)];
                (cell.c, cell.flags.bits()).hash(&mut hasher);
                hash_color(cell.fg, &mut hasher);
                hash_color(cell.bg, &mut hasher);
                if let Some(zerowidth) = cell.zerowidth() {
                    zerowidth.hash(&mut hasher);
                }
            }
        }
        let cursor = grid.cursor.point;
        (cursor.line.0, cursor.column.0, self.term.mode().contains(TermMode::SHOW_CURSOR)).hash(&mut hasher);
        hasher.finish()
    }

    pub fn title(&self) -> Option<String> {
        self.events.0.lock().unwrap().title.clone()
    }

    pub fn take_printed(&mut self) -> String {
        std::mem::take(&mut self.stream.printed)
    }

    pub fn take_shell_reports(&mut self) -> Vec<(usize, ShellReport)> {
        std::mem::take(&mut self.stream.reports)
    }

    pub fn reported_cwd(&self) -> Option<String> {
        self.stream.cwd.clone()
    }

    pub fn grid(&self) -> Grid {
        let grid = self.term.grid();
        let cols = self.size.cols as usize;
        let rows = (0..self.size.rows as i32)
            .map(|line| (0..cols).map(|col| glyph(&grid[Line(line)][Column(col)])).collect())
            .collect();
        let point = grid.cursor.point;
        let shown = self.term.mode().contains(TermMode::SHOW_CURSOR) && point.line.0 >= 0;
        Grid { rows, cursor: shown.then_some((point.line.0 as usize, point.column.0)) }
    }

    pub fn view(&self) -> ScreenView {
        let grid = self.term.grid();
        let lines = self.rows(0..self.size.rows as i32, false);
        let point = grid.cursor.point;
        let mode = *self.term.mode();
        let mouse = mouse_encoding(mode);
        ScreenView {
            size: self.size,
            lines,
            cursor: (point.line.0.max(0) as u16, point.column.0 as u16),
            cursor_visible: mode.contains(TermMode::SHOW_CURSOR),
            alt_screen: mode.contains(TermMode::ALT_SCREEN),
            mouse,
        }
    }
}
