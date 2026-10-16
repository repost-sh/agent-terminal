use super::Status;
use super::state::State;
use crate::protocol::Size;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Glance {
    pub frame: u64,
    pub size: Size,
    pub alt_screen: bool,
    pub mouse: Option<&'static str>,
    pub bracketed_paste: bool,
    pub status: Status,
}

impl Glance {
    pub(super) fn of(state: &State) -> Self {
        let State { screen, frame, status, .. } = state;
        Self {
            frame: *frame,
            size: screen.size(),
            alt_screen: screen.alt_screen(),
            mouse: screen.mouse().map(crate::native::screen::MouseMode::as_str),
            bracketed_paste: screen.keyboard_modes().bracketed_paste,
            status: *status,
        }
    }
}

pub(crate) struct Picture {
    pub glance: Glance,
    pub rows: Vec<String>,
    pub cursor: (u16, u16),
    pub cursor_visible: bool,
}

impl Picture {
    pub(super) fn of(state: &State) -> Self {
        let glance = Glance::of(state);
        let screen = &state.screen;
        let (line, column) = screen.cursor();
        Self {
            rows: screen.rows(0..i32::from(glance.size.rows), true),
            cursor: (line + 1, column + 1),
            cursor_visible: screen.cursor_visible(),
            glance,
        }
    }

    pub(crate) fn paint(&self, shown: Option<&[String]>) -> String {
        let mut paint = String::new();
        for (index, row) in self.rows.iter().enumerate() {
            if shown.is_none_or(|shown| shown.get(index) != Some(row)) {
                paint.push_str(&crate::paint::row(index, row));
            }
        }
        paint.push_str(&crate::paint::cursor(
            [self.cursor.0, self.cursor.1],
            self.cursor_visible && !self.glance.status.exited(),
        ));
        paint
    }
}
