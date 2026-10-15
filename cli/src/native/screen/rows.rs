use alacritty_terminal::grid::Row;
use alacritty_terminal::index::Column;
use alacritty_terminal::term::cell::{Cell as GridCell, Flags};
use alacritty_terminal::vte::ansi::{Color, NamedColor};

pub(super) fn is_spacer(cell: &GridCell) -> bool {
    cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
}

pub(super) fn push_cell(text: &mut String, cell: &GridCell) {
    text.push(cell.c);
    if let Some(zerowidth) = cell.zerowidth() {
        text.extend(zerowidth);
    }
}

pub(super) fn plain_row(row: &Row<GridCell>, cols: u16) -> String {
    located_row(row, cols).0
}

pub(super) fn located_row(row: &Row<GridCell>, cols: u16) -> (String, Vec<u16>) {
    let mut text = String::with_capacity(usize::from(cols));
    let mut columns = Vec::with_capacity(usize::from(cols));
    for col in (0..cols).filter(|&col| !is_spacer(&row[Column(usize::from(col))])) {
        push_cell(&mut text, &row[Column(usize::from(col))]);
        columns.resize(text.len(), col);
    }
    text.truncate(text.trim_end().len());
    columns.truncate(text.len());
    (text, columns)
}

fn shows(cell: &GridCell) -> bool {
    cell.c != ' '
        || cell.bg != Color::Named(NamedColor::Background)
        || cell.flags.intersects(Flags::INVERSE | Flags::ALL_UNDERLINES | Flags::STRIKEOUT)
}

fn switch_to(style: &str) -> String {
    if style.is_empty() { "\x1b[0m".to_string() } else { format!("\x1b[0;{style}m") }
}

pub(super) fn styled_row(row: &Row<GridCell>, cols: u16) -> String {
    let end = (0..usize::from(cols)).rposition(|col| shows(&row[Column(col)])).map_or(0, |i| i + 1);
    let mut text = String::new();
    let mut current = String::new();
    for cell in (0..end).map(|col| &row[Column(col)]).filter(|cell| !is_spacer(cell)) {
        let style = sgr(cell);
        if style != current {
            text.push_str(&switch_to(&style));
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
