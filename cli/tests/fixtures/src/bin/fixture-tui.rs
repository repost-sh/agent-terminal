use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind,
};
use crossterm::execute;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, List, ListState, Paragraph};

const ITEMS: [&str; 4] = ["alpha.txt", "beta.txt", "gamma.txt", "delta.txt"];
const YES: std::ops::Range<u16> = 0..7;
const NO: std::ops::Range<u16> = 9..15;

#[derive(Default)]
struct Areas {
    list: Rect,
    buttons: Rect,
}

fn main() -> std::io::Result<()> {
    let mut mouse = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--mouse" => mouse = true,
            other => panic!("unknown argument: {other}"),
        }
    }
    let mut terminal = ratatui::init();
    if mouse {
        execute!(std::io::stdout(), EnableMouseCapture)?;
    }
    let mut state = ListState::default().with_selected(Some(0));
    let mut dialog = false;
    let mut status: Option<String> = None;
    let mut areas = Areas::default();
    let code = loop {
        let selected = ITEMS[state.selected().unwrap_or(0).min(ITEMS.len() - 1)];
        terminal.draw(|frame| {
            let area = frame.area();
            let [body, hints] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
            let [files, details] = Layout::horizontal([Constraint::Percentage(40), Constraint::Fill(1)]).areas(body);
            let list = List::new(ITEMS)
                .block(Block::bordered().title("Files"))
                .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
            frame.render_stateful_widget(list, files, &mut state);
            areas.list = Block::bordered().inner(files);
            if dialog {
                let block = Block::bordered().title("Delete");
                let inner = block.inner(details);
                areas.buttons = Rect { x: inner.x, y: inner.y + 2, width: inner.width, height: 1 };
                frame.render_widget(
                    Paragraph::new(format!("Delete {selected}?\n\n[ Yes ]  [ No ]")).block(block),
                    details,
                );
            } else {
                let mut text = format!("Selected: {selected}\nSize: {}x{}", area.width, area.height);
                if let Some(status) = &status {
                    text.push_str(&format!("\n{status}"));
                }
                frame.render_widget(Paragraph::new(text).block(Block::bordered().title("Details")), details);
            }
            frame.render_widget(Line::from("Move: j/k | Quit: q | Crash: x"), hints);
        })?;
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match (dialog, key.code) {
                (true, KeyCode::Char('y')) => (dialog, status) = (false, Some(format!("Deleted: {selected}"))),
                (true, KeyCode::Char('n') | KeyCode::Esc) => {
                    (dialog, status) = (false, Some(format!("Kept: {selected}")))
                }
                (true, _) => {}
                (false, KeyCode::Char('j') | KeyCode::Down) => state.select_next(),
                (false, KeyCode::Char('k') | KeyCode::Up) => state.select_previous(),
                (false, KeyCode::PageDown) => state.select(Some(ITEMS.len() - 1)),
                (false, KeyCode::PageUp) => state.select(Some(0)),
                (false, KeyCode::Char('d')) => dialog = true,
                (false, KeyCode::Char('q')) => break 0,
                (false, KeyCode::Char('x')) => break 3,
                (false, _) => {}
            },
            Event::Mouse(event) => {
                let at = Position { x: event.column, y: event.row };
                match event.kind {
                    MouseEventKind::Down(button) => {
                        status = Some(format!("Mouse: {} {},{}", name(button), at.y + 1, at.x + 1));
                        let on_buttons = dialog && areas.buttons.contains(at);
                        let offset = at.x.saturating_sub(areas.buttons.x);
                        match button {
                            MouseButton::Left if on_buttons && YES.contains(&offset) => {
                                (dialog, status) = (false, Some(format!("Deleted: {selected}")))
                            }
                            MouseButton::Left if on_buttons && NO.contains(&offset) => {
                                (dialog, status) = (false, Some(format!("Kept: {selected}")))
                            }
                            MouseButton::Left if !dialog && areas.list.contains(at) => {
                                state.select(Some(usize::from(at.y - areas.list.y)))
                            }
                            _ => {}
                        }
                    }
                    MouseEventKind::ScrollDown if !dialog => state.select_next(),
                    MouseEventKind::ScrollUp if !dialog => state.select_previous(),
                    _ => {}
                }
            }
            _ => {}
        }
        if state.selected().is_some_and(|i| i >= ITEMS.len()) {
            state.select(Some(ITEMS.len() - 1));
        }
    };
    if mouse {
        execute!(std::io::stdout(), DisableMouseCapture)?;
    }
    ratatui::restore();
    std::process::exit(code);
}

fn name(button: MouseButton) -> &'static str {
    match button {
        MouseButton::Left => "left",
        MouseButton::Right => "right",
        MouseButton::Middle => "middle",
    }
}
