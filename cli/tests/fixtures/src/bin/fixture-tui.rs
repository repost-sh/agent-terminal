use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, List, ListState, Paragraph};

const ITEMS: [&str; 4] = ["alpha.txt", "beta.txt", "gamma.txt", "delta.txt"];

fn main() -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    let mut state = ListState::default().with_selected(Some(0));
    let code = loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let [body, hints] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
            let [files, details] = Layout::horizontal([Constraint::Percentage(40), Constraint::Fill(1)]).areas(body);
            let list = List::new(ITEMS)
                .block(Block::bordered().title("Files"))
                .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
            frame.render_stateful_widget(list, files, &mut state);
            let selected = ITEMS[state.selected().unwrap_or(0)];
            frame.render_widget(
                Paragraph::new(format!("Selected: {selected}\nSize: {}x{}", area.width, area.height))
                    .block(Block::bordered().title("Details")),
                details,
            );
            frame.render_widget(Line::from("Move: j/k | Quit: q | Crash: x"), hints);
        })?;
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => state.select_next(),
                KeyCode::Char('k') | KeyCode::Up => state.select_previous(),
                KeyCode::Char('q') => break 0,
                KeyCode::Char('x') => break 3,
                _ => {}
            }
            if state.selected().is_some_and(|i| i >= ITEMS.len()) {
                state.select(Some(ITEMS.len() - 1));
            }
        }
    };
    ratatui::restore();
    std::process::exit(code);
}
