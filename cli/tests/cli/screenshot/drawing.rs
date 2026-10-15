use super::picture::{
    BACKGROUND, BLUE, Cell, FOREGROUND, GREEN, MAGENTA_RGB, Picture, RED, XTERM_208, assert_filled, assert_lines,
    picture,
};
use super::screen::Screen;
use crate::support::AgentTerminal;

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
    let screen = Screen::open(&at, "44x8", EVERYTHING, "end");
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
        assert!(shot.cell(6, col).inked(0..9, 0..20, BACKGROUND), "the emoji take two cells each: col {col}");
    }
    the_powerline_arrow_and_the_nerd_folder_are_drawn(&shot);
}

fn the_picture_has_its_margin_and_empty_cells(shot: &Picture) {
    assert_eq!((shot.width, shot.height), (44 * 9 + 2 * 10, 8 * 20 + 2 * 10));
    assert_eq!(shot.at(0, 0), BACKGROUND, "the margin");
    assert_filled(&shot.cell(8, 44), 0..9, 0..20, BACKGROUND, "an empty cell");
    assert_filled(&shot.cell(7, 29), 0..9, 0..20, BACKGROUND, "the hidden cursor's cell");
}

fn inverse_text_is_drawn_in_the_background_colour(shot: &Picture) {
    for col in 3..=11 {
        let inverse = shot.cell(2, col);
        assert_filled(&inverse, 0..9, 0..2, FOREGROUND, "the inverse row's backdrop");
        assert!(inverse.has(BACKGROUND), "the inverse text at col {col} is drawn in the background color");
    }
}

fn colours_are_drawn_as_set(shot: &Picture) {
    assert!(shot.cell(5, 1).has(RED), "\"red\" is red");
    assert!(shot.cell(5, 5).has(GREEN), "\"green\" is green");
    assert_filled(&shot.cell(5, 16), 0..9, 0..20, BLUE, "the space before \"blue\" on a blue background");
    assert!(shot.cell(5, 23).has(XTERM_208), "\"256\" is xterm color 208");
    assert!(shot.cell(5, 27).has(MAGENTA_RGB), "\"rgb\" is #ff00ff");
}

fn box_drawing_lines_reach_the_edges_they_join(shot: &Picture) {
    assert_lines(&shot.cell(1, 1), [false, true, true, false], "┌");
    assert_lines(&shot.cell(1, 2), [false, true, false, true], "─");
    assert_lines(&shot.cell(1, 12), [false, false, true, true], "┐");
    assert_lines(&shot.cell(2, 1), [true, false, true, false], "│");
    assert_lines(&shot.cell(3, 1), [true, true, true, false], "├");
    assert_lines(&shot.cell(4, 1), [true, true, false, false], "└");
    assert_lines(&shot.cell(1, 14), [false, true, true, false], "╭");
    assert_lines(&shot.cell(4, 14), [false, true, true, false], "┏");
    assert_lines(&shot.cell(7, 19), [true, true, true, true], "┼");
    assert_lines(&shot.cell(7, 20), [true, true, true, true], "╋");
    assert_lines(&shot.cell(7, 21), [true, true, true, true], "╬");
}

fn blocks_fill_their_part_of_the_cell(shot: &Picture) {
    assert_filled(&shot.cell(7, 8), 0..9, 0..20, FOREGROUND, "█ fills its cell");
    assert_filled(&shot.cell(7, 4), 0..9, 0..9, BACKGROUND, "▄ leaves the upper half");
    assert_filled(&shot.cell(7, 4), 0..9, 11..20, FOREGROUND, "▄ fills the lower half");
    assert_filled(&shot.cell(7, 1), 0..9, 0..17, BACKGROUND, "▁ leaves all but the lowest eighth");
    assert_filled(&shot.cell(7, 1), 0..9, 19..20, FOREGROUND, "▁ fills the lowest eighth");
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
        assert_filled(cell, xs, ys, color, block);
    }
}

fn the_powerline_arrow_and_the_nerd_folder_are_drawn(shot: &Picture) {
    let arrow = shot.cell(6, 29);
    assert!(arrow.inked(0..2, 9..11, BACKGROUND), "the powerline arrow  starts at its cell's left edge");
    assert!(arrow.inked(3..6, 9..11, BACKGROUND), "the powerline arrow  is solid, not an empty box");
    assert!(!arrow.inked(7..9, 0..2, BACKGROUND), "the powerline arrow  narrows to a point");
    assert!(shot.cell(6, 27).inked(0..9, 0..20, BACKGROUND), "the nerd font folder  is drawn");
}
