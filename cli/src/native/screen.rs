use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::Processor;

use crate::protocol::Size;

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

const KITTY_FLAGS: [(TermMode, u8); 5] = [
    (TermMode::DISAMBIGUATE_ESC_CODES, 1),
    (TermMode::REPORT_EVENT_TYPES, 2),
    (TermMode::REPORT_ALTERNATE_KEYS, 4),
    (TermMode::REPORT_ALL_KEYS_AS_ESC, 8),
    (TermMode::REPORT_ASSOCIATED_TEXT, 16),
];

#[derive(Clone, Default)]
struct Replies(Arc<Mutex<Vec<u8>>>);

impl EventListener for Replies {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event {
            self.0.lock().unwrap().extend_from_slice(text.as_bytes());
        }
    }
}

pub struct Screen {
    term: Term<Replies>,
    parser: Processor,
    replies: Replies,
    size: Size,
}

impl Screen {
    pub fn new(size: Size) -> Self {
        let replies = Replies::default();
        let config = Config { kitty_keyboard: true, ..Config::default() };
        Self { term: Term::new(config, &size, replies.clone()), parser: Processor::new(), replies, size }
    }

    pub fn keyboard_modes(&self) -> KeyboardModes {
        let mode = *self.term.mode();
        KeyboardModes {
            app_cursor: mode.contains(TermMode::APP_CURSOR),
            bracketed_paste: mode.contains(TermMode::BRACKETED_PASTE),
            kitty_flags: KITTY_FLAGS.iter().filter(|(flag, _)| mode.contains(*flag)).map(|(_, bit)| bit).sum(),
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.parser.advance(&mut self.term, bytes);
        std::mem::take(&mut *self.replies.0.lock().unwrap())
    }

    pub fn view(&self) -> ScreenView {
        let grid = self.term.grid();
        let lines = (0..self.size.rows as i32)
            .map(|row| {
                let row = &grid[Line(row)];
                let mut text = String::with_capacity(self.size.cols as usize);
                for col in 0..self.size.cols as usize {
                    let cell = &row[Column(col)];
                    if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                        continue;
                    }
                    text.push(cell.c);
                    if let Some(zerowidth) = cell.zerowidth() {
                        text.extend(zerowidth);
                    }
                }
                text.truncate(text.trim_end().len());
                text
            })
            .collect();
        let point = grid.cursor.point;
        let mode = *self.term.mode();
        let mouse = mode.intersects(TermMode::MOUSE_MODE).then(|| {
            if mode.contains(TermMode::SGR_MOUSE) {
                MouseMode::Sgr
            } else if mode.contains(TermMode::UTF8_MOUSE) {
                MouseMode::Utf8
            } else {
                MouseMode::X10
            }
        });
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
