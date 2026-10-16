use super::{Decoration, Emphasis, Found, Glyph, Grid, Link, find};
use crate::native::screen::Colour;

fn glyph(text: &str) -> Glyph {
    Glyph {
        text: text.to_string(),
        fg: Colour::Default,
        bg: Colour::Default,
        emphasis: Emphasis { bold: false, italic: false, dim: false },
        decoration: Decoration { underline: false, strikeout: false },
        inverse: false,
        wide: false,
        link: None,
    }
}

fn screen(lines: &[&str]) -> Grid {
    let rows = lines.iter().map(|line| line.chars().map(|c| glyph(&c.to_string())).collect()).collect();
    Grid { rows, cursor: None }
}

fn listed(grid: &Grid) -> String {
    find(grid)
        .unwrap()
        .iter()
        .map(|found: &Found| {
            let area = found.area();
            let url = found.url().map_or(String::new(), |url| format!(" {url}"));
            format!(
                "{} {:?} at {},{} over {}-{} {}-{}{url}",
                found.kind(),
                found.label(),
                found.row(),
                found.col(),
                area.row,
                area.last_row,
                area.col,
                area.last_col
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn lines(grid: &Grid) -> Vec<String> {
    listed(grid).lines().map(str::to_string).collect()
}

#[test]
fn a_titled_box_is_a_panel_spanning_down_to_its_corner_or_its_last_side() {
    let boxes = screen(&[
        "┌─ Files ─┐ ╭─────╮ ┌──x",
        "│ a.txt   │ │ x   │ │",
        "└─────────┘ ╰─────╯ │",
        "╔═Logs════╗ ┌─ y ─┐",
        "║         ║ x",
        "║         ║",
    ]);
    assert_eq!(lines(&boxes), ["panel \"Files\" at 0,3 over 0-2 0-10", "panel \"Logs\" at 3,2 over 3-5 0-10",]);
}

#[test]
fn a_bracketed_or_angled_label_is_a_button_when_it_has_a_letter_and_at_most_24_cells() {
    let row = screen(&["[ OK ] < Cancel > [ - ] [ abcdefghijklmnopqrstuvwxy ] [ abcdefghijklmnopqrstuvwx ] [x]"]);
    assert_eq!(
        lines(&row),
        [
            "button \"OK\" at 0,2 over 0-0 0-5",
            "button \"Cancel\" at 0,9 over 0-0 7-16",
            "button \"abcdefghijklmnopqrstuvwx\" at 0,56 over 0-0 54-81",
        ]
    );
}

#[test]
fn key_hints_count_only_when_a_row_has_two_and_not_where_a_button_already_is() {
    let rows = screen(&["q: quit  ?: help • ^C cancel | Enter: open", "q: quit", "[ q: quit ]  x: close  y: yank"]);
    assert_eq!(
        lines(&rows),
        [
            "hint \"q: quit\" at 0,0 over 0-0 0-6",
            "hint \"?: help\" at 0,9 over 0-0 9-15",
            "hint \"^C cancel\" at 0,19 over 0-0 19-27",
            "hint \"Enter: open\" at 0,31 over 0-0 31-41",
            "button \"q: quit\" at 2,2 over 2-2 0-10",
            "hint \"x: close\" at 2,13 over 2-2 13-20",
            "hint \"y: yank\" at 2,23 over 2-2 23-29",
        ]
    );
}

#[test]
fn a_run_on_an_uncommon_backdrop_is_selected_unless_other_refs_cover_its_letters() {
    let mut rows = screen(&["  open file   ", " x   ", "[ OK ]  q: quit  w: write"]);
    for col in 2..11 {
        rows.rows[0][col].inverse = true;
    }
    rows.rows[1][1].bg = Colour::Indexed(4);
    for col in 0..6 {
        rows.rows[2][col].bg = Colour::Indexed(2);
    }
    for col in 8..15 {
        rows.rows[2][col].inverse = true;
        rows.rows[2][col].fg = Colour::Indexed(3);
    }
    assert_eq!(
        lines(&rows),
        [
            "selected \"open file\" at 0,2 over 0-0 2-10",
            "button \"OK\" at 2,2 over 2-2 0-5",
            "hint \"q: quit\" at 2,8 over 2-2 8-14",
            "hint \"w: write\" at 2,17 over 2-2 17-24",
        ]
    );
}

#[test]
fn the_cursor_marks_the_input_between_the_nearest_sides_or_across_the_row() {
    let mut boxed = screen(&["plain │ cursor here │ end", "     "]);
    boxed.cursor = Some((0, 10));
    assert_eq!(lines(&boxed), ["input \"cursor here\" at 0,10 over 0-0 7-19"]);
    boxed.cursor = Some((1, 2));
    assert_eq!(lines(&boxed), ["input \"\" at 1,2 over 1-1 0-4"]);
    boxed.cursor = Some((0, 1));
    assert_eq!(lines(&boxed), ["input \"plain\" at 0,1 over 0-0 0-5"]);
    boxed.cursor = Some((4, 0));
    assert!(lines(&boxed).is_empty());
}

#[test]
fn each_linked_run_is_a_link_to_its_url() {
    let mut row = screen(&["docs api  "]);
    let link = |url: &str| Some(Link { id: url.to_string(), url: url.to_string() });
    for col in 0..4 {
        row.rows[0][col].link = link("https://a.dev");
    }
    for col in 4..8 {
        row.rows[0][col].link = link("https://b.dev");
    }
    row.rows[0][8].link = link("https://c.dev");
    assert_eq!(
        lines(&row),
        ["link \"docs\" at 0,0 over 0-0 0-3 https://a.dev", "link \"api\" at 0,5 over 0-0 4-7 https://b.dev"]
    );
}

#[test]
fn a_ref_after_a_wide_or_multibyte_letter_keeps_its_column() {
    let mut row = screen(&["é中 [ OK ]"]);
    row.rows[0][1].wide = true;
    assert_eq!(lines(&row), ["button \"OK\" at 0,5 over 0-0 3-8"]);
}

#[test]
fn a_found_ref_is_still_on_screen_only_while_its_area_is_unchanged() {
    let mut grid = screen(&["[ OK ] x"]);
    let found = find(&grid).unwrap();
    assert!(found[0].still_on(&grid));
    grid.rows[0][7].text = "y".to_string();
    assert!(found[0].still_on(&grid));
    grid.rows[0][2].text = "N".to_string();
    assert!(!found[0].still_on(&grid));
    grid.rows.clear();
    assert!(!found[0].still_on(&grid));
}
