mod colour;
mod modes;
mod rows;
mod search;
mod stream;

use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;
use std::sync::{Arc, Mutex, PoisonError};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Row};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell as GridCell, Flags};
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::Parser;
use alacritty_terminal::vte::ansi::{Color, Processor};

pub(crate) use colour::Colour;
use modes::{KITTY_FLAGS, mouse_encoding, mouse_tracking};
pub(crate) use modes::{KeyboardModes, MouseMode, MouseReporting, MouseTracking};
use rows::{is_spacer, located_row, plain_row, push_cell, styled_row};
use stream::Stream;
pub(crate) use stream::{At, Printed, ShellReport};

use crate::native::refs::{Decoration, Emphasis, Glyph, Grid, Link};
use crate::protocol::{Cell, Modes, Mouse, Size};

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        usize::from(self.rows)
    }

    fn screen_lines(&self) -> usize {
        usize::from(self.rows)
    }

    fn columns(&self) -> usize {
        usize::from(self.cols)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ScreenView {
    pub size: Size,
    pub lines: Vec<String>,
    pub cursor: (u16, u16),
    pub cursor_visible: bool,
    pub alt_screen: bool,
    pub mouse: Option<MouseMode>,
}

fn glyph(cell: &GridCell) -> Glyph {
    let mut text = String::new();
    if !is_spacer(cell) {
        push_cell(&mut text, cell);
    }
    Glyph {
        text,
        fg: cell.fg.into(),
        bg: cell.bg.into(),
        emphasis: Emphasis {
            bold: cell.flags.contains(Flags::BOLD),
            italic: cell.flags.contains(Flags::ITALIC),
            dim: cell.flags.contains(Flags::DIM),
        },
        decoration: Decoration {
            underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
            strikeout: cell.flags.contains(Flags::STRIKEOUT),
        },
        inverse: cell.flags.contains(Flags::INVERSE),
        wide: cell.flags.contains(Flags::WIDE_CHAR),
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

fn hash_row(row: &Row<GridCell>, cols: u16, hasher: &mut DefaultHasher) {
    for col in 0..usize::from(cols) {
        let cell = &row[Column(col)];
        (cell.c, cell.flags.bits()).hash(hasher);
        hash_color(cell.fg, hasher);
        hash_color(cell.bg, hasher);
        if let Some(zerowidth) = cell.zerowidth() {
            zerowidth.hash(hasher);
        }
    }
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
        let mut reported = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match event {
            Event::PtyWrite(text) => reported.replies.extend_from_slice(text.as_bytes()),
            Event::Title(title) => reported.title = Some(title),
            Event::ResetTitle => reported.title = None,
            _ => {}
        }
    }
}

pub(crate) struct Screen {
    term: Term<Events>,
    parser: Processor,
    events: Events,
    stream_parser: Parser,
    stream: Stream,
    size: Size,
}

impl Screen {
    pub(crate) fn new(size: Size) -> Self {
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

    pub(crate) fn keyboard_modes(&self) -> KeyboardModes {
        let mode = *self.term.mode();
        KeyboardModes {
            app_cursor: mode.contains(TermMode::APP_CURSOR),
            bracketed_paste: mode.contains(TermMode::BRACKETED_PASTE),
            kitty_flags: KITTY_FLAGS.iter().filter(|(flag, ..)| mode.contains(*flag)).map(|(_, bit, _)| bit).sum(),
        }
    }

    pub(crate) fn size(&self) -> Size {
        self.size
    }

    pub(crate) fn resize(&mut self, size: Size) {
        self.term.resize(size);
        self.size = size;
    }

    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.parser.advance(&mut self.term, bytes);
        self.stream_parser.advance(&mut self.stream, bytes);
        std::mem::take(&mut self.events.0.lock().unwrap_or_else(PoisonError::into_inner).replies)
    }

    pub(crate) fn scrollback_len(&self) -> usize {
        self.term.grid().history_size()
    }

    pub(crate) fn rows(&self, lines: Range<i32>, ansi: bool) -> Vec<String> {
        let grid = self.term.grid();
        let cols = self.size.cols;
        lines
            .map(|line| if ansi { styled_row(&grid[Line(line)], cols) } else { plain_row(&grid[Line(line)], cols) })
            .collect()
    }

    pub(crate) fn located_row(&self, line: i32) -> (String, Vec<u16>) {
        located_row(&self.term.grid()[Line(line)], self.size.cols)
    }

    pub(crate) fn mouse_reporting(&self) -> Option<MouseReporting> {
        let mode = *self.term.mode();
        mouse_tracking(mode).zip(mouse_encoding(mode)).map(|(tracking, encoding)| MouseReporting { tracking, encoding })
    }

    pub(crate) fn modes(&self) -> Modes {
        let mode = *self.term.mode();
        Modes {
            alt_screen: mode.contains(TermMode::ALT_SCREEN).into(),
            mouse: self.mouse_reporting().map(|reporting| Mouse {
                tracking: reporting.tracking.as_str().to_string(),
                encoding: reporting.encoding.as_str().to_string(),
            }),
            bracketed_paste: mode.contains(TermMode::BRACKETED_PASTE).into(),
            app_cursor: mode.contains(TermMode::APP_CURSOR).into(),
            focus_events: mode.contains(TermMode::FOCUS_IN_OUT).into(),
            keyboard: KITTY_FLAGS
                .iter()
                .filter(|(flag, ..)| mode.contains(*flag))
                .map(|(.., name)| name.to_string())
                .collect(),
        }
    }

    pub(crate) fn cell(&self, row: u16, col: u16) -> Option<Cell> {
        if !(1..=self.size.rows).contains(&row) || !(1..=self.size.cols).contains(&col) {
            return None;
        }
        let cell = &self.term.grid()[Line(i32::from(row) - 1)][Column(usize::from(col) - 1)];
        let mut char = cell.c.to_string();
        if let Some(zerowidth) = cell.zerowidth() {
            char.extend(zerowidth);
        }
        Some(Cell {
            row,
            col,
            char,
            fg: Colour::from(cell.fg).to_string(),
            bg: Colour::from(cell.bg).to_string(),
            bold: cell.flags.contains(Flags::BOLD),
            inverse: cell.flags.contains(Flags::INVERSE),
            underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
        })
    }

    pub(crate) fn fingerprint(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        let grid = self.term.grid();
        for line in 0..i32::from(self.size.rows) {
            hash_row(&grid[Line(line)], self.size.cols, &mut hasher);
        }
        let cursor = grid.cursor.point;
        (cursor.line.0, cursor.column.0, self.term.mode().contains(TermMode::SHOW_CURSOR)).hash(&mut hasher);
        hasher.finish()
    }

    pub(crate) fn title(&self) -> Option<String> {
        self.events.0.lock().unwrap_or_else(PoisonError::into_inner).title.clone()
    }

    pub(crate) fn take_printed(&mut self) -> Printed {
        Printed { text: std::mem::take(&mut self.stream.printed), styled: std::mem::take(&mut self.stream.styled) }
    }

    pub(crate) fn take_shell_reports(&mut self) -> Vec<(At, ShellReport)> {
        std::mem::take(&mut self.stream.reports)
    }

    pub(crate) fn reported_cwd(&self) -> Option<String> {
        self.stream.cwd.clone()
    }

    pub(crate) fn grid(&self) -> Grid {
        let grid = self.term.grid();
        let cols = usize::from(self.size.cols);
        let rows = (0..i32::from(self.size.rows))
            .map(|line| (0..cols).map(|col| glyph(&grid[Line(line)][Column(col)])).collect())
            .collect();
        let point = grid.cursor.point;
        let line = usize::try_from(point.line.0).ok().filter(|_| self.cursor_visible());
        Grid { rows, cursor: line.map(|line| (line, point.column.0)) }
    }

    pub(crate) fn view(&self) -> ScreenView {
        ScreenView {
            size: self.size,
            lines: self.rows(0..i32::from(self.size.rows), false),
            cursor: self.cursor(),
            cursor_visible: self.cursor_visible(),
            alt_screen: self.alt_screen(),
            mouse: self.mouse(),
        }
    }

    pub(crate) fn cursor(&self) -> (u16, u16) {
        let point = self.term.grid().cursor.point;
        let line = u16::try_from(point.line.0.max(0)).unwrap_or(u16::MAX);
        (line, u16::try_from(point.column.0).unwrap_or(u16::MAX))
    }

    pub(crate) fn cursor_visible(&self) -> bool {
        self.term.mode().contains(TermMode::SHOW_CURSOR)
    }

    pub(crate) fn alt_screen(&self) -> bool {
        self.term.mode().contains(TermMode::ALT_SCREEN)
    }

    pub(crate) fn mouse(&self) -> Option<MouseMode> {
        mouse_encoding(*self.term.mode())
    }
}
