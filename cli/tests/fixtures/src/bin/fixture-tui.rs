use std::error::Error;
use std::process::ExitCode;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton, MouseEvent,
    MouseEventKind,
};
use crossterm::execute;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, List, ListState, Paragraph};
use ratatui::{DefaultTerminal, Frame};

const ITEMS: [&str; 4] = ["alpha.txt", "beta.txt", "gamma.txt", "delta.txt"];
const YES: std::ops::Range<u16> = 0..7;
const NO: std::ops::Range<u16> = 9..15;

#[derive(Default)]
struct Areas {
    list: Rect,
    buttons: Rect,
}

struct App {
    state: ListState,
    dialog: bool,
    status: Option<String>,
    areas: Areas,
}

fn main() -> Result<ExitCode, Box<dyn Error>> {
    let mouse = mouse_wanted(std::env::args().skip(1))?;
    let mut terminal = ratatui::init();
    if mouse {
        execute!(std::io::stdout(), EnableMouseCapture)?;
    }
    let code = App::new().run(&mut terminal)?;
    if mouse {
        execute!(std::io::stdout(), DisableMouseCapture)?;
    }
    ratatui::restore();
    Ok(ExitCode::from(code))
}

fn mouse_wanted(args: impl Iterator<Item = String>) -> Result<bool, String> {
    let mut mouse = false;
    for arg in args {
        match arg.as_str() {
            "--mouse" => mouse = true,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(mouse)
}

impl App {
    fn new() -> Self {
        Self {
            state: ListState::default().with_selected(Some(0)),
            dialog: false,
            status: None,
            areas: Areas::default(),
        }
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<u8> {
        loop {
            let selected = self.selected();
            terminal.draw(|frame| self.draw(frame, selected))?;
            if let Some(code) = self.handle(&event::read()?, selected) {
                return Ok(code);
            }
            if self.state.selected().is_some_and(|i| i >= ITEMS.len()) {
                self.state.select(Some(ITEMS.len() - 1));
            }
        }
    }

    fn selected(&self) -> &'static str {
        let [.., last] = ITEMS;
        ITEMS.get(self.state.selected().unwrap_or(0)).copied().unwrap_or(last)
    }

    fn draw(&mut self, frame: &mut Frame<'_>, selected: &str) {
        let area = frame.area();
        let [body, hints] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
        let [files, details] = Layout::horizontal([Constraint::Percentage(40), Constraint::Fill(1)]).areas(body);
        let list = List::new(ITEMS)
            .block(Block::bordered().title("Files"))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
        frame.render_stateful_widget(list, files, &mut self.state);
        self.areas.list = Block::bordered().inner(files);
        if self.dialog {
            self.draw_dialog(frame, details, selected);
        } else {
            self.draw_details(frame, details, selected);
        }
        frame.render_widget(Line::from("Move: j/k | Quit: q | Crash: x"), hints);
    }

    fn draw_dialog(&mut self, frame: &mut Frame<'_>, details: Rect, selected: &str) {
        let block = Block::bordered().title("Delete");
        let inner = block.inner(details);
        self.areas.buttons = Rect { x: inner.x, y: inner.y + 2, width: inner.width, height: 1 };
        frame.render_widget(Paragraph::new(format!("Delete {selected}?\n\n[ Yes ]  [ No ]")).block(block), details);
    }

    fn draw_details(&self, frame: &mut Frame<'_>, details: Rect, selected: &str) {
        let area = frame.area();
        let status = self.status.as_deref().map_or(String::new(), |status| format!("\n{status}"));
        let text = format!("Selected: {selected}\nSize: {}x{}{status}", area.width, area.height);
        frame.render_widget(Paragraph::new(text).block(Block::bordered().title("Details")), details);
    }

    fn handle(&mut self, event: &Event, selected: &str) -> Option<u8> {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => return self.key(key.code, selected),
            Event::Mouse(event) => self.mouse(*event, selected),
            _ => {}
        }
        None
    }

    fn key(&mut self, code: KeyCode, selected: &str) -> Option<u8> {
        if self.dialog {
            self.answer(code, selected);
            return None;
        }
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.state.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.state.select_previous(),
            KeyCode::PageDown => self.state.select(Some(ITEMS.len() - 1)),
            KeyCode::PageUp => self.state.select(Some(0)),
            KeyCode::Char('d') => self.dialog = true,
            KeyCode::Char('q') => return Some(0),
            KeyCode::Char('x') => return Some(3),
            _ => {}
        }
        None
    }

    fn answer(&mut self, code: KeyCode, selected: &str) {
        match code {
            KeyCode::Char('y') => self.close_dialog(format!("Deleted: {selected}")),
            KeyCode::Char('n') | KeyCode::Esc => self.close_dialog(format!("Kept: {selected}")),
            _ => {}
        }
    }

    fn close_dialog(&mut self, status: String) {
        self.dialog = false;
        self.status = Some(status);
    }

    fn mouse(&mut self, event: MouseEvent, selected: &str) {
        let at = Position { x: event.column, y: event.row };
        match event.kind {
            MouseEventKind::Down(button) => {
                self.status = Some(format!("Mouse: {} {},{}", name(button), at.y + 1, at.x + 1));
                if button == MouseButton::Left {
                    self.left_click(at, selected);
                }
            }
            MouseEventKind::ScrollDown if !self.dialog => self.state.select_next(),
            MouseEventKind::ScrollUp if !self.dialog => self.state.select_previous(),
            _ => {}
        }
    }

    fn left_click(&mut self, at: Position, selected: &str) {
        if self.dialog {
            self.press_button(at, selected);
        } else if self.areas.list.contains(at) {
            self.state.select(Some(usize::from(at.y - self.areas.list.y)));
        }
    }

    fn press_button(&mut self, at: Position, selected: &str) {
        if !self.areas.buttons.contains(at) {
            return;
        }
        let offset = at.x.saturating_sub(self.areas.buttons.x);
        if YES.contains(&offset) {
            self.close_dialog(format!("Deleted: {selected}"));
        } else if NO.contains(&offset) {
            self.close_dialog(format!("Kept: {selected}"));
        }
    }
}

fn name(button: MouseButton) -> &'static str {
    match button {
        MouseButton::Left => "left",
        MouseButton::Right => "right",
        MouseButton::Middle => "middle",
    }
}
