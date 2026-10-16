use tiny_skia::Pixmap;

use super::fonts::CellSize;
use super::theme::{Rgb, palette};
use super::{
    Canvas, Cells, Drawing, Entry, Image, Layout, Placed, RowRange, Span, blended, cell_colors, cells, check_fonts,
    image, legend_lines, offset, pixel, shifted, span_of,
};
use crate::native::refs::{Decoration, Emphasis, Glyph, Grid};
use crate::native::screen::Colour;
use crate::protocol::{Look, Ref, RefKind, Theme};

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

fn grid(lines: &[&str]) -> Grid {
    let rows = lines.iter().map(|line| line.chars().map(|c| glyph(&c.to_string())).collect()).collect();
    Grid { rows, cursor: None }
}

fn reference(id: &str, rows: [u16; 2], cols: [u16; 2]) -> Ref {
    Ref {
        id: id.to_string(),
        kind: RefKind::Button,
        label: "ok".to_string(),
        row: rows[0],
        col: cols[0],
        rows,
        cols,
        frame: 1,
        url: None,
    }
}

fn rgb(image: &Image, x: u32, y: u32) -> Rgb {
    let pixel = image.pixmap.pixel(x, y).unwrap().demultiply();
    Rgb(pixel.red(), pixel.green(), pixel.blue())
}

fn plain(scale: u8, cursor: bool) -> Look {
    Look { scale, theme: Theme::Dark, cursor }
}

fn drawn(grid: &Grid, refs: &[Ref]) -> Image {
    image(&Drawing { grid, rows: RowRange { first: 1, last: 99 }, look: plain(1, true) }, refs).unwrap()
}

fn cell() -> CellSize {
    CellSize::at(1).unwrap()
}

#[test]
fn the_rows_shown_are_clamped_to_the_screen_and_counted_from_one() {
    let screen = grid(&["abcd", "efgh", "ijkl"]);
    let shown = |first, last| {
        let Span { cols, first, last } = Span::of(&screen, RowRange { first, last }).unwrap();
        (cols, first, last)
    };
    assert_eq!([shown(2, 3), shown(0, 2), shown(9, 12), shown(3, 1)], [(4, 1, 3), (4, 0, 2), (4, 3, 3), (4, 2, 2)]);
    assert_eq!(Span::of(&Grid { rows: Vec::new(), cursor: None }, RowRange { first: 1, last: 2 }).unwrap().cols, 0);
    let drawing = Drawing { grid: &screen, rows: RowRange { first: 2, last: 3 }, look: plain(0, false) };
    assert_eq!(cells(&drawing).unwrap(), Cells { scale: 1, cols: 4, rows: RowRange { first: 2, last: 3 } });
    let scaled = Drawing { look: plain(3, false), ..drawing };
    assert_eq!(cells(&scaled).unwrap().scale, 3);
}

#[test]
fn the_layout_places_cells_after_the_padding_and_rows_from_the_first_shown() {
    let screen = grid(&["abcd", "efgh", "ijkl"]);
    let layout = Layout::new(&screen, RowRange { first: 2, last: 3 }, 2).unwrap();
    let cell = CellSize::at(2).unwrap();
    let (w, h) = (u32::from(cell.width), u32::from(cell.height));
    assert_eq!((layout.pad, layout.x(0), layout.x(3), layout.y(1), layout.y(2)), (20, 20, 20 + 3 * w, 20, 20 + h));
    assert_eq!(
        (layout.cells_wide(5), layout.grid_width(), layout.grid_height(), layout.legend_top()),
        (5 * w, 4 * w, 2 * h, 40 + 2 * h)
    );
    assert_eq!(Layout::new(&screen, RowRange { first: 1, last: 3 }, 0).unwrap().pad, 10);
}

#[test]
fn a_ref_shows_when_it_reaches_a_row_in_view() {
    let screen = grid(&["a", "b", "c", "d"]);
    let layout = Layout::new(&screen, RowRange { first: 2, last: 3 }, 1).unwrap();
    let shows = |rows| layout.shows(&reference("1", rows, [1, 1]));
    assert_eq!(
        [shows([1, 1]), shows([1, 2]), shows([3, 3]), shows([4, 4]), shows([3, 5])],
        [false, true, true, false, true]
    );
}

#[test]
fn the_picture_holds_the_grid_its_padding_and_a_legend_at_least_twenty_cells_wide() {
    let screen = grid(&["ab", "cd"]);
    let layout = Layout::new(&screen, RowRange { first: 1, last: 2 }, 1).unwrap();
    let (w, h) = (u32::from(cell().width), u32::from(cell().height));
    let entry = || Entry { index: 0, col: 0, tag: "1".to_string(), rest: String::new() };
    assert_eq!(layout.picture_size(&[]), (2 * w + 20, 2 * h + 20));
    assert_eq!(layout.picture_size(&[vec![entry()], vec![entry()]]), (20 * w + 20, 2 * h + 20 + 10 + 2 * h));
    let wide = grid(&["abcdefghijklmnopqrstuvwxyz"]);
    let layout = Layout::new(&wide, RowRange { first: 1, last: 1 }, 1).unwrap();
    assert_eq!(layout.picture_size(&[vec![entry()]]).0, 26 * w + 20);
}

#[test]
fn legend_entries_fill_a_line_three_cells_apart_and_wrap_when_the_next_would_not_fit() {
    let refs = [reference("1", [1, 1], [1, 1]), reference("22", [1, 1], [1, 1]), reference("3", [1, 1], [1, 1])];
    let shown: Vec<(usize, &Ref)> = refs.iter().enumerate().collect();
    let placed = |lines: Vec<Vec<Entry>>| -> Vec<Vec<(usize, u16, String, String)>> {
        lines
            .into_iter()
            .map(|line| line.into_iter().map(|entry| (entry.index, entry.col, entry.tag, entry.rest)).collect())
            .collect()
    };
    let rest = "button \"ok\"".to_string();
    assert_eq!(
        placed(legend_lines(&shown, 30)),
        [
            vec![(0, 0, "1".to_string(), rest.clone()), (1, 16, "22".to_string(), rest.clone())],
            vec![(2, 0, "3".to_string(), rest.clone())]
        ]
    );
    let lines = |cols| legend_lines(&shown, cols).len();
    assert_eq!([lines(29), lines(30), lines(45), lines(46)], [3, 2, 2, 1]);
    assert_eq!(placed(legend_lines(&shown[..1], 5)), [vec![(0, 0, "1".to_string(), rest)]]);
    assert!(legend_lines(&[], 20).is_empty());
}

#[test]
fn a_cell_swaps_its_colours_when_inverse_or_under_the_cursor_and_dims_toward_its_background() {
    let colours = palette(Theme::Dark);
    let (fg, bg) = (colours.foreground, colours.background);
    let coloured = |change: fn(&mut Glyph), cursor| {
        let mut styled = glyph("a");
        change(&mut styled);
        cell_colors(&styled, colours, cursor)
    };
    assert_eq!(
        [
            coloured(|_| {}, false),
            coloured(|_| {}, true),
            coloured(|styled| styled.inverse = true, false),
            coloured(|styled| styled.inverse = true, true),
            coloured(|styled| styled.emphasis.dim = true, false),
            coloured(|styled| styled.emphasis.dim = true, true),
        ],
        [(fg, bg), (bg, fg), (bg, fg), (fg, bg), (fg.mix(bg, 0.4), bg), (bg, fg.mix(bg, 0.4))]
    );
    let set = |styled: &mut Glyph| {
        styled.fg = Colour::Indexed(1);
        styled.bg = Colour::Rgb(1, 2, 3);
    };
    assert_eq!(coloured(set, false), (colours.ansi[1], Rgb(1, 2, 3)));
}

#[test]
fn a_wide_glyph_spans_two_cells_unless_it_is_in_the_last_column() {
    let mut wide = glyph("中");
    wide.wide = true;
    assert_eq!(
        [span_of(&wide, 0, 3), span_of(&wide, 1, 3), span_of(&wide, 2, 3), span_of(&glyph("a"), 0, 3)],
        [2, 2, 1, 1]
    );
}

#[test]
fn a_picture_has_the_layouts_size_and_the_themes_background() {
    let screen = grid(&["ab"]);
    let picture = drawn(&screen, &[]);
    let (w, h) = (u32::from(cell().width), u32::from(cell().height));
    assert_eq!((picture.width(), picture.height()), (2 * w + 20, h + 20));
    assert_eq!(rgb(&picture, 0, 0), palette(Theme::Dark).background);
    let light = image(
        &Drawing {
            grid: &screen,
            rows: RowRange { first: 1, last: 1 },
            look: Look { theme: Theme::Light, ..plain(1, false) },
        },
        &[],
    )
    .unwrap();
    assert_eq!(rgb(&light, 0, 0), palette(Theme::Light).background);
}

#[test]
fn a_cell_paints_its_background_and_its_letter_in_its_colours() {
    let (w, h) = (u32::from(cell().width), u32::from(cell().height));
    let colours = palette(Theme::Dark);
    let mut screen = grid(&["  "]);
    screen.rows[0][0].bg = Colour::Indexed(4);
    screen.rows[0][1].text = "W".to_string();
    let picture = drawn(&screen, &[]);
    let corners = [rgb(&picture, 10, 10), rgb(&picture, 10 + w - 1, 10 + h - 1), rgb(&picture, 10 + w, 10)];
    assert_eq!(corners, [colours.ansi[4], colours.ansi[4], colours.background]);
    let letter = (0..w)
        .flat_map(|x| (0..h).map(move |y| (x, y)))
        .filter(|&(x, y)| rgb(&picture, 10 + w + x, 10 + y) != colours.background)
        .count();
    assert!(letter > 10, "the W covers {letter} pixels");
}

#[test]
fn an_underline_and_a_strikeout_are_one_stroke_across_the_cell() {
    let (w, cell) = (u32::from(cell().width), cell());
    let colours = palette(Theme::Dark);
    let mut screen = grid(&["  "]);
    screen.rows[0][0].decoration = Decoration { underline: true, strikeout: false };
    screen.rows[0][1].decoration = Decoration { underline: false, strikeout: true };
    let picture = drawn(&screen, &[]);
    let (under, through, stroke) = (u32::from(cell.underline), u32::from(cell.strikeout), u32::from(cell.stroke));
    assert_eq!(
        [
            rgb(&picture, 10, 10 + under),
            rgb(&picture, 10 + w - 1, 10 + under + stroke - 1),
            rgb(&picture, 10, 10 + under + stroke),
            rgb(&picture, 10 + w, 10 + through),
            rgb(&picture, 10 + 2 * w - 1, 10 + through),
            rgb(&picture, 10 + w, 10 + through + stroke),
            rgb(&picture, 10 + w, 10 + under),
        ],
        [
            colours.foreground,
            colours.foreground,
            colours.background,
            colours.foreground,
            colours.foreground,
            colours.background,
            colours.background
        ]
    );
}

#[test]
fn the_cursor_shows_only_when_the_look_asks_for_it() {
    let (w, h) = (u32::from(cell().width), u32::from(cell().height));
    let colours = palette(Theme::Dark);
    let mut screen = grid(&[" ", " "]);
    screen.cursor = Some((1, 0));
    let shown = |cursor| {
        image(&Drawing { grid: &screen, rows: RowRange { first: 1, last: 2 }, look: plain(1, cursor) }, &[]).unwrap()
    };
    assert_eq!(rgb(&shown(true), 10, 10 + h), colours.foreground);
    assert_eq!(rgb(&shown(true), 10, 10), colours.background);
    assert_eq!(rgb(&shown(false), 10 + w / 2, 10 + h + 1), colours.background);
}

#[test]
fn a_letter_no_font_has_is_drawn_as_an_outlined_box() {
    let colours = palette(Theme::Dark);
    let cell = cell();
    let picture = drawn(&grid(&["\u{10fffd}"]), &[]);
    let (left, top) = (10 + u32::from(cell.stroke), 10 + u32::from(cell.height) / 5);
    assert_eq!(rgb(&picture, left, top), colours.foreground);
    assert_eq!(rgb(&picture, left, 10 + u32::from(cell.baseline) - 1), colours.foreground);
    assert_eq!(rgb(&picture, left + 1, top + 1 + u32::from(cell.stroke)), colours.background);
}

#[test]
fn a_ref_is_outlined_and_tagged_in_its_colour_and_listed_in_the_legend() {
    let colours = palette(Theme::Dark);
    let (w, h) = (u32::from(cell().width), u32::from(cell().height));
    let screen = grid(&["    ", "    ", "    "]);
    let refs = [reference("1", [2, 2], [2, 3]), reference("2", [3, 3], [1, 1])];
    let picture = drawn(&screen, &refs);
    assert_eq!(rgb(&picture, 10 + w, 10 + h + h / 2), colours.refs[0]);
    assert_eq!(rgb(&picture, 10 + w + w / 2, 10 + h + h / 2), colours.background);
    assert_eq!(rgb(&picture, 10, 10 + 2 * h + h / 2), colours.refs[1]);
    let legend = 20 + 3 * h;
    assert_eq!((rgb(&picture, 10, legend), rgb(&picture, 10, legend + h)), (colours.refs[0], colours.refs[1]));
    assert_eq!(picture.height(), legend + 2 * h + 10);
}

#[test]
fn a_ref_outside_the_rows_shown_is_neither_outlined_nor_listed() {
    let screen = grid(&["  ", "  "]);
    let refs = [reference("1", [2, 2], [1, 1])];
    let rows = RowRange { first: 1, last: 1 };
    let picture = image(&Drawing { grid: &screen, rows, look: plain(1, false) }, &refs).unwrap();
    assert_eq!(picture.height(), u32::from(cell().height) + 20);
}

#[test]
fn a_fill_stays_inside_the_picture() {
    let mut canvas = Canvas::new(4, 3, Rgb(0, 0, 0)).unwrap();
    canvas.fill(2, 1, 10, 10, Rgb(9, 8, 7));
    let painted: Vec<u8> = canvas.pixmap.data().chunks(4).map(|pixel| pixel[0]).collect();
    assert_eq!(painted, [0, 0, 0, 0, 0, 0, 9, 9, 0, 0, 9, 9]);
    assert_eq!(Canvas::new(0, 0, Rgb(0, 0, 0)).unwrap().pixmap.width(), 1);
}

#[test]
fn a_blend_mixes_each_channel_by_the_masks_coverage() {
    let mixed = |under: u8, alpha| {
        let mut pixel = [under, under, under, 255];
        blended(&mut pixel, Rgb(255, 100, 0), alpha);
        pixel
    };
    assert_eq!(
        [mixed(0, 0), mixed(0, 255), mixed(0, 128), mixed(200, 51)],
        [[0, 0, 0, 255], [255, 100, 0, 255], [128, 50, 0, 255], [211, 180, 160, 255]]
    );
}

#[test]
fn pixel_positions_shift_by_a_masks_offset_and_stay_inside_the_buffer() {
    assert_eq!([shifted(5, -3, 1), shifted(0, -1, 0), shifted(2, 3, 4)], [Some(3), None, Some(9)]);
    assert_eq!([offset(10, 2, 3), offset(10, 0, 0), offset(u32::MAX, u32::MAX, u32::MAX)], [Some(128), Some(0), None]);
    let mut data = vec![0u8; 2 * 2 * 4];
    assert_eq!(pixel(&mut data, 2, 1, 1).map(|pixel| pixel.len()), Some(4));
    assert!(pixel(&mut data, 2, 0, 2).is_none());
}

#[test]
fn a_ref_box_spans_its_cells_with_its_tag_above_its_right_end_when_there_is_room() {
    let screen = grid(&["    ", "    ", "    "]);
    let layout = Layout::new(&screen, RowRange { first: 1, last: 3 }, 1).unwrap();
    let (w, h) = (u32::from(cell().width), u32::from(cell().height));
    let placed = Placed::new(&layout, &reference("12", [2, 3], [2, 3])).unwrap();
    let tag = &placed.tag;
    assert_eq!(
        (placed.left, placed.top, placed.right, placed.bottom, placed.line, tag.width),
        (10 + w, 10 + h, 10 + 3 * w, 10 + 3 * h, 2, 2 * u32::from(tag.cell.width) + 4)
    );
    assert_eq!((tag.x, tag.y), (10 + 3 * w - tag.width, 10 + h - u32::from(tag.cell.height)));
    let (cw, ch) = (f32::from(cell().width), f32::from(cell().height));
    assert_eq!(placed.corners(), Some([11.0 + cw, 11.0 + ch, 9.0 + 3.0 * cw, 9.0 + 3.0 * ch]));
    let top = Placed::new(&layout, &reference("1", [1, 1], [1, 1])).unwrap();
    assert_eq!(top.tag.y, 10);
}

#[test]
fn the_bundled_fonts_cover_every_face() {
    assert_eq!(check_fonts(), Ok(vec!["JetBrains Mono", "Symbols Nerd Font Mono", "Noto Emoji"]));
}

#[test]
fn a_picture_encodes_as_an_rgb_png_of_its_pixels() {
    let mut canvas = Canvas::new(2, 1, Rgb(1, 2, 3)).unwrap();
    canvas.fill(1, 0, 1, 1, Rgb(200, 100, 50));
    let picture = Image { pixmap: canvas.pixmap };
    let decoded = Pixmap::decode_png(&picture.png().unwrap()).unwrap();
    let pixels: Vec<u8> = decoded.data().to_vec();
    assert_eq!((decoded.width(), decoded.height(), pixels), (2, 1, vec![1, 2, 3, 255, 200, 100, 50, 255]));
}

#[test]
fn a_legend_tag_is_as_many_cells_wide_as_its_ref_has_letters() {
    let colours = palette(Theme::Dark);
    let (w, h) = (u32::from(cell().width), u32::from(cell().height));
    let picture = drawn(&grid(&["  "]), &[reference("12", [1, 1], [1, 1])]);
    let bottom = 20 + 2 * h - 1;
    assert_eq!(
        [rgb(&picture, 10 + 2 * w - 1, bottom), rgb(&picture, 10 + 2 * w, bottom)],
        [colours.refs[0], colours.background]
    );
}

#[test]
fn the_missing_letter_box_stops_one_stroke_short_of_the_cells_right_edge() {
    let colours = palette(Theme::Dark);
    let cell = cell();
    let picture = drawn(&grid(&["\u{10fffd}"]), &[]);
    let (line, width) = (u32::from(cell.stroke), u32::from(cell.width));
    let (right, top, bottom) = (10 + width - 2 * line, 10 + u32::from(cell.height) / 5, 10 + u32::from(cell.baseline));
    assert_eq!(
        [
            rgb(&picture, right - 1, top),
            rgb(&picture, right, top),
            rgb(&picture, right - line, bottom - 2),
            rgb(&picture, right - line - 1, bottom - 2)
        ],
        [colours.foreground, colours.background, colours.foreground, colours.background]
    );
}

#[test]
fn a_cell_with_no_letter_draws_nothing() {
    let colours = palette(Theme::Dark);
    let (w, h) = (u32::from(cell().width), u32::from(cell().height));
    let picture = drawn(&grid(&[""]), &[]);
    let mut empty = grid(&[" "]);
    empty.rows[0][0].text = String::new();
    let blank = drawn(&empty, &[]);
    let painted = (0..w)
        .flat_map(|x| (0..h).map(move |y| (x, y)))
        .filter(|&(x, y)| rgb(&blank, 10 + x, 10 + y) != colours.background)
        .count();
    assert_eq!((picture.width(), painted), (20, 0));
}

#[test]
fn the_missing_letter_box_outlines_the_letter_area_at_any_place_and_stroke() {
    let cell = CellSize::at(2).unwrap();
    let (line, height, baseline) = (u32::from(cell.stroke), u32::from(cell.height), u32::from(cell.baseline));
    let (x, y, width) = (7, 5, 2 * u32::from(cell.width));
    let mut canvas = Canvas::new(60, 60, Rgb(0, 0, 0)).unwrap();
    canvas.missing(x, y, width, &cell, Rgb(255, 255, 255));
    let (left, right, top, bottom) = (x + line, x + width - 2 * line, y + height / 5, y + baseline);
    let outlined = |px: u32, py: u32| {
        let across = (left..right).contains(&px);
        let down = (top..bottom).contains(&py);
        (across && ((top..top + line).contains(&py) || (bottom - line..bottom).contains(&py)))
            || (down && ((left..left + line).contains(&px) || (right - line..right).contains(&px)))
    };
    let painted: Vec<(u32, u32)> = (0..60)
        .flat_map(|py| (0..60).map(move |px| (px, py)))
        .filter(|&(px, py)| canvas.pixmap.pixel(px, py).unwrap().red() == 255)
        .collect();
    let expected: Vec<(u32, u32)> =
        (0..60).flat_map(|py| (0..60).map(move |px| (px, py))).filter(|&(px, py)| outlined(px, py)).collect();
    assert_eq!((line, painted.len()), (2, expected.len()));
    assert_eq!(painted, expected);
}

#[test]
fn a_legend_entry_writes_its_tag_over_the_fill_and_its_kind_and_label_after_it() {
    let colours = palette(Theme::Dark);
    let (w, h) = (u32::from(cell().width), u32::from(cell().height));
    let picture = drawn(&grid(&["  "]), &[reference("1", [1, 1], [1, 1])]);
    let top = 20 + h;
    let differing = |from: u32, cells: u32, ground: Rgb| {
        (from..from + cells * w)
            .flat_map(|x| (top..top + h).map(move |y| (x, y)))
            .filter(|&(x, y)| rgb(&picture, x, y) != ground)
            .count()
    };
    let (tag, button, space) = (
        differing(10, 1, colours.refs[0]),
        differing(10 + 2 * w, 6, colours.background),
        differing(10 + 8 * w, 1, colours.background),
    );
    assert!(tag > 10 && button > 100 && space == 0, "tag {tag}, button {button}, space {space}");
}
