use super::picture::{BACKGROUND, BLUE, Cell, FOREGROUND, GREEN, MAGENTA_RGB, Picture, RED, XTERM_208, picture};
use crate::support::{AgentTerminal, Screen, Shown};

const EVERYTHING: &str = "\x1b[?25l\
┌─ Files ──┐ ╭─ Log ──╮ ╔═ Help ═╗\n\
│ \x1b[7malpha.txt\x1b[0m│ │ ok     │ ║ q quit ║\n\
├──────────┤ ╰────────╯ ╠════════╣\n\
└──────────┘ ┏━━┓ ┃x┃   ╚════════╝\n\
\x1b[31mred\x1b[0m \x1b[32mgreen\x1b[0m \x1b[1;33mbold\x1b[0m \x1b[44;37m blue \x1b[0m \x1b[38;5;208m256\x1b[0m \x1b[38;2;255;0;255mrgb\x1b[0m \x1b[3mitalic\x1b[0m \x1b[4munder\x1b[0m\n\
漢字 wide · 😀 🚀 emoji · \u{f07b} \u{e0b0} nerd\n\
▁▂▃▄▅▆▇█ ░▒▓ ▖▗▘▝ ┼╋╬ ╱╲ end";

#[test]
fn a_screenshot_draws_colors_box_drawing_blocks_emoji_and_symbols_in_their_cells() {
    let at = AgentTerminal::new();
    let screen = Screen::open(&at, &Shown { size: "44x8", bytes: EVERYTHING, wait: "end" });
    let path = screen.file("everything.png");

    at.run(&["screenshot", &path.to_string_lossy()]).success();

    let shot = picture(&path);
    the_picture_has_its_margin_and_empty_cells(&shot);
    inverse_text_is_drawn_in_the_background_colour(&shot);
    colours_are_drawn_as_set(&shot);
    box_drawing_lines_reach_the_edges_they_join(&shot);
    blocks_fill_their_part_of_the_cell(&shot);
    quadrants_fill_their_quarter(&shot);
    let shades: Vec<u32> = (10..=12).map(|col| shot.cell(7, col).brightness()).collect();
    assert!(shades[0] < shades[1] && shades[1] < shades[2], "░ ▒ ▓ get darker to lighter: {shades:?}");
    for col in [13, 14, 16, 17] {
        assert!(shot.cell(6, col).patch(0..9, 0..20).inked(BACKGROUND), "the emoji take two cells each: col {col}");
    }
    the_powerline_arrow_and_the_nerd_folder_are_drawn(&shot);
}

fn the_picture_has_its_margin_and_empty_cells(shot: &Picture) {
    assert_eq!((shot.width, shot.height), (44 * 9 + 2 * 10, 8 * 20 + 2 * 10));
    assert_eq!(shot.at(0, 0), BACKGROUND, "the margin");
    shot.cell(8, 44).patch(0..9, 0..20).assert_filled(BACKGROUND, "an empty cell");
    shot.cell(7, 29).patch(0..9, 0..20).assert_filled(BACKGROUND, "the hidden cursor's cell");
}

fn inverse_text_is_drawn_in_the_background_colour(shot: &Picture) {
    for col in 3..=11 {
        let inverse = shot.cell(2, col);
        inverse.patch(0..9, 0..2).assert_filled(FOREGROUND, "the inverse row's backdrop");
        assert!(inverse.has(BACKGROUND), "the inverse text at col {col} is drawn in the background color");
    }
}

fn colours_are_drawn_as_set(shot: &Picture) {
    assert!(shot.cell(5, 1).has(RED), "\"red\" is red");
    assert!(shot.cell(5, 5).has(GREEN), "\"green\" is green");
    shot.cell(5, 16).patch(0..9, 0..20).assert_filled(BLUE, "the space before \"blue\" on a blue background");
    assert!(shot.cell(5, 23).has(XTERM_208), "\"256\" is xterm color 208");
    assert!(shot.cell(5, 27).has(MAGENTA_RGB), "\"rgb\" is #ff00ff");
}

fn box_drawing_lines_reach_the_edges_they_join(shot: &Picture) {
    shot.cell(1, 1).assert_lines([false, true, true, false], "┌");
    shot.cell(1, 2).assert_lines([false, true, false, true], "─");
    shot.cell(1, 12).assert_lines([false, false, true, true], "┐");
    shot.cell(2, 1).assert_lines([true, false, true, false], "│");
    shot.cell(3, 1).assert_lines([true, true, true, false], "├");
    shot.cell(4, 1).assert_lines([true, true, false, false], "└");
    shot.cell(1, 14).assert_lines([false, true, true, false], "╭");
    shot.cell(4, 14).assert_lines([false, true, true, false], "┏");
    shot.cell(7, 19).assert_lines([true, true, true, true], "┼");
    shot.cell(7, 20).assert_lines([true, true, true, true], "╋");
    shot.cell(7, 21).assert_lines([true, true, true, true], "╬");
}

fn blocks_fill_their_part_of_the_cell(shot: &Picture) {
    shot.cell(7, 8).patch(0..9, 0..20).assert_filled(FOREGROUND, "█ fills its cell");
    shot.cell(7, 4).patch(0..9, 0..9).assert_filled(BACKGROUND, "▄ leaves the upper half");
    shot.cell(7, 4).patch(0..9, 11..20).assert_filled(FOREGROUND, "▄ fills the lower half");
    shot.cell(7, 1).patch(0..9, 0..17).assert_filled(BACKGROUND, "▁ leaves all but the lowest eighth");
    shot.cell(7, 1).patch(0..9, 19..20).assert_filled(FOREGROUND, "▁ fills the lowest eighth");
}

fn quadrants_fill_their_quarter(shot: &Picture) {
    let quadrants = [("▖", 14, [false, false, true, false]), ("▗", 15, [false, false, false, true])];
    let quadrants =
        quadrants.into_iter().chain([("▘", 16, [true, false, false, false]), ("▝", 17, [false, true, false, false])]);
    for (block, col, filled) in quadrants {
        the_quadrant_fills_its_quarter(&shot.cell(7, col), block, filled);
    }
}

fn the_quadrant_fills_its_quarter(cell: &Cell<'_>, block: &str, filled: [bool; 4]) {
    let parts = [(0..4, 0..9), (5..9, 0..9), (0..4, 11..20), (5..9, 11..20)];
    for ((xs, ys), on) in parts.into_iter().zip(filled) {
        let color = if on { FOREGROUND } else { BACKGROUND };
        cell.patch(xs, ys).assert_filled(color, block);
    }
}

fn the_powerline_arrow_and_the_nerd_folder_are_drawn(shot: &Picture) {
    let arrow = shot.cell(6, 29);
    assert!(arrow.patch(0..2, 9..11).inked(BACKGROUND), "the powerline arrow  starts at its cell's left edge");
    assert!(arrow.patch(3..6, 9..11).inked(BACKGROUND), "the powerline arrow  is solid, not an empty box");
    assert!(!arrow.patch(7..9, 0..2).inked(BACKGROUND), "the powerline arrow  narrows to a point");
    assert!(shot.cell(6, 27).patch(0..9, 0..20).inked(BACKGROUND), "the nerd font folder  is drawn");
}

#[test]
fn strikeout_draws_a_line_across_the_whole_cell() {
    let at = AgentTerminal::new();
    let screen = Screen::open(&at, &Shown { size: "12x2", bytes: "\x1b[?25l\x1b[9mstrike\x1b[0m end", wait: "end" });
    let path = screen.file("strike.png");
    at.run(&["screenshot", &path.to_string_lossy()]).success();
    let shot = picture(&path);
    let full_rows = |col| {
        (0..20).filter(|&y| (0..9).all(|x| shot.cell(1, col).patch(x..x + 1, y..y + 1).inked(BACKGROUND))).count()
    };
    assert_eq!((1..=6).map(full_rows).collect::<Vec<_>>(), [1; 6], "each struck letter has one full-width row");
    assert_eq!((8..=10).map(full_rows).collect::<Vec<_>>(), [0; 3], "the plain letters have none");
}
