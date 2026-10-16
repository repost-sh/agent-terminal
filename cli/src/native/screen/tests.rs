use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};

use super::{Colour, KeyboardModes, MouseMode, MouseReporting, MouseTracking, Screen, ShellReport};
use crate::protocol::{Cell, Size};

fn screen(cols: u16, rows: u16, bytes: &str) -> Screen {
    let mut screen = Screen::new(Size { cols, rows });
    screen.feed(bytes.as_bytes());
    screen
}

const STYLED: &str = "\x1b[1;31mred\x1b[0m plain \x1b[4munder\x1b[0m\r\n\x1b[38;5;196mx\x1b[48;2;1;2;3my\x1b[0m\x1b[92mz\x1b[0m\x1b[2;7;8;9mq\x1b[0m\x1b[3m\x1b[44m  \x1b[0m\r\n中e\u{301}  ";

#[test]
fn rows_read_as_plain_text_without_trailing_blanks_or_with_their_styles() {
    let styled = screen(20, 3, STYLED);
    assert_eq!(styled.rows(0..3, false), ["red plain under", "xyzq", "中e\u{301}"]);
    assert_eq!(
        styled.rows(0..3, true),
        [
            "\x1b[0;1;31mred\x1b[0m plain \x1b[0;4munder\x1b[0m",
            "\x1b[0;38;5;196mx\x1b[0;38;5;196;48;2;1;2;3my\x1b[0;92mz\x1b[0;2;7;8;9mq\x1b[0;3;44m  \x1b[0m",
            "中e\u{301}"
        ]
    );
    assert_eq!(styled.located_row(2), ("中e\u{301}".to_string(), vec![0, 0, 0, 2, 2, 2]));
}

#[test]
fn bright_and_background_colours_have_their_own_codes() {
    let colours = screen(20, 1, "\x1b[90ma\x1b[97;101mb\x1b[0;42mc\x1b[0;1;37md\x1b[0;39;49me");
    assert_eq!(colours.rows(0..1, true), ["\x1b[0;90ma\x1b[0;97;101mb\x1b[0;42mc\x1b[0;1;37md\x1b[0me"]);
    assert_eq!(screen(4, 1, "ab").rows(0..1, true), ["ab"]);
}

#[test]
fn a_cell_has_its_letter_marks_colours_and_flags_inside_the_screen_only() {
    let styled = screen(20, 3, STYLED);
    let cell = |row, col, char: &str, (fg, bg): (&str, &str)| Cell {
        row,
        col,
        char: char.to_string(),
        fg: fg.to_string(),
        bg: bg.to_string(),
        bold: false,
        inverse: false,
        underline: false,
    };
    let json = |found: Option<Cell>| serde_json::to_value(found).unwrap();
    assert_eq!(json(styled.cell(1, 1)), json(Some(Cell { bold: true, ..cell(1, 1, "r", ("red", "default")) })));
    assert_eq!(
        json(styled.cell(1, 11)),
        json(Some(Cell { underline: true, ..cell(1, 11, "u", ("default", "default")) }))
    );
    assert_eq!(json(styled.cell(2, 2)), json(Some(cell(2, 2, "y", ("196", "#010203")))));
    assert_eq!(json(styled.cell(2, 4)), json(Some(Cell { inverse: true, ..cell(2, 4, "q", ("default", "default")) })));
    assert_eq!(json(styled.cell(3, 3)), json(Some(cell(3, 3, "e\u{301}", ("default", "default")))));
}

#[test]
fn a_cell_outside_the_screen_is_refused_and_one_at_its_edge_is_not() {
    let styled = screen(20, 3, STYLED);
    assert!([styled.cell(0, 1), styled.cell(4, 1), styled.cell(1, 0), styled.cell(1, 21)].iter().all(Option::is_none));
    assert_eq!(styled.cell(3, 20).map(|found| found.char), Some(" ".to_string()));
    assert_eq!(styled.cell_at(4, 1).err(), Some("cell 4,1 is outside the 20x3 screen".to_string()));
    assert_eq!(styled.cell_at(1, 2).map(|found| found.char).ok(), Some("e".to_string()));
}

#[test]
fn the_grid_carries_each_glyphs_style_its_link_and_a_visible_cursor() {
    let mut drawn = screen(10, 2, "\x1b[1;2;3;4;7;9ma\x1b[0m\x1b]8;id=x;https://a.dev\x07l\x1b]8;;\x07中");
    let grid = drawn.grid();
    let first = &grid.rows[0][0];
    assert_eq!(
        (
            first.emphasis.bold,
            first.emphasis.dim,
            first.emphasis.italic,
            first.decoration.underline,
            first.inverse,
            first.decoration.strikeout
        ),
        (true, true, true, true, true, true)
    );
    let link = grid.rows[0][1].link.clone().unwrap();
    assert_eq!(
        (link.id.ends_with('x'), link.url.as_str(), grid.rows[0][2].wide, grid.rows[0][3].text.as_str()),
        (true, "https://a.dev", true, "")
    );
    assert_eq!((grid.cursor, grid.rows[0][4].text.as_str(), grid.rows[0][0].wide), (Some((0, 4)), " ", false));
    drawn.feed(b"\x1b[?25l");
    assert_eq!(drawn.grid().cursor, None);
}

#[test]
fn a_query_is_answered_and_a_title_kept_until_reset() {
    let mut replies = Screen::new(Size { cols: 10, rows: 4 });
    assert_eq!(replies.feed(b"ab\x1b[6n\x1b[c"), b"\x1b[1;3R\x1b[?6c");
    assert_eq!(replies.feed(b"x"), b"");
    replies.feed(b"\x1b[22t\x1b]2;my title\x07");
    assert_eq!(replies.title(), Some("my title".to_string()));
    replies.feed(b"\x1b[23t");
    assert_eq!(replies.title(), None);
}

#[test]
fn the_modes_the_program_turns_on_are_reported_by_name() {
    let mut modes = Screen::new(Size { cols: 10, rows: 4 });
    modes.feed(b"\x1b[?1h\x1b[?2004h\x1b[?1002h\x1b[?1006h\x1b[?1004h\x1b[?1049h\x1b[>31u\x1b[?25l");
    assert_eq!(modes.keyboard_modes(), KeyboardModes { app_cursor: true, bracketed_paste: true, kitty_flags: 31 });
    let reported = modes.modes();
    assert_eq!(
        (reported.keyboard, reported.mouse.map(|mouse| (mouse.tracking, mouse.encoding))),
        (
            vec!["disambiguate", "report-events", "report-alternates", "report-all-keys", "report-text"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            Some(("drag".to_string(), "sgr".to_string()))
        )
    );
    let view = modes.view();
    assert_eq!(
        (view.alt_screen, view.cursor_visible, view.mouse, view.lines.len(), view.size),
        (true, false, Some(MouseMode::Sgr), 4, Size { cols: 10, rows: 4 })
    );
    assert_eq!(
        serde_json::to_value(modes.modes()).unwrap()["focus_events"],
        serde_json::to_value(modes.modes().alt_screen).unwrap()
    );
    assert_eq!(Screen::new(Size { cols: 2, rows: 1 }).keyboard_modes(), KeyboardModes::default());
}

#[test]
fn mouse_reporting_names_its_tracking_and_its_encoding() {
    let reporting = |bytes: &str| screen(4, 1, bytes).mouse_reporting();
    assert_eq!(
        [
            reporting(""),
            reporting("\x1b[?1000h"),
            reporting("\x1b[?1003h\x1b[?1005h"),
            reporting("\x1b[?1002h\x1b[?1006h")
        ],
        [
            None,
            Some(MouseReporting { tracking: MouseTracking::Click, encoding: MouseMode::X10 }),
            Some(MouseReporting { tracking: MouseTracking::Motion, encoding: MouseMode::Utf8 }),
            Some(MouseReporting { tracking: MouseTracking::Drag, encoding: MouseMode::Sgr })
        ]
    );
    assert_eq!([MouseMode::X10.as_str(), MouseMode::Sgr.as_str(), MouseMode::Utf8.as_str()], ["x10", "sgr", "utf8"]);
    assert_eq!(
        [MouseTracking::Click.as_str(), MouseTracking::Drag.as_str(), MouseTracking::Motion.as_str()],
        ["click", "drag", "motion"]
    );
}

#[test]
fn printed_text_keeps_newlines_tabs_and_colour_changes_and_drops_other_controls() {
    let mut printed = screen(
        20,
        4,
        "\x1b]7;file://host/tmp/a%20b%2\x07$ \x1b[1Als\r\n\x1b[32;4:3mfile\x1b[0m\tx\x07\x1b[2J\x1b[?1h\r\n",
    );
    let out = printed.take_printed();
    assert_eq!((out.text.as_str(), out.styled.as_str()), ("$ ls\nfile\tx\n", "$ ls\n\x1b[32;4:3mfile\x1b[0m\tx\n"));
    assert_eq!((printed.take_printed().text, printed.reported_cwd()), (String::new(), Some("/tmp/a b%2".to_string())));
    let mut moved = screen(20, 2, "\x1b]7;file://host/var\x07\x1b]7;nonsense\x07\x1b]7;file://host\x07");
    assert_eq!(moved.reported_cwd(), Some("/var".to_string()));
    assert_eq!(moved.take_printed().text, "");
}

#[test]
fn shell_marks_report_the_prompt_the_command_and_its_exit_at_their_place_in_the_output() {
    let mut shell = screen(
        30,
        4,
        "\x1b]7;file://h/tmp\x07\x1b]133;A\x07$ \x1b]133;B\x07ls\r\n\x1b]133;C;cmdline_url=ls%20-la\x07\x1b[32mok\x1b[0m\r\n\x1b]133;D;3\x07\x1b]133;C;cmdline=echo hi\x07\x1b]133;D\x07\x1b]133;C\x07\x1b]133;Z\x07\x1b]133;D;x\x07",
    );
    let reports: Vec<_> =
        shell.take_shell_reports().into_iter().map(|(at, report)| (at.text, at.styled, report)).collect();
    let started = |command: &str| ShellReport::Started { command: command.to_string(), cwd: Some("/tmp".to_string()) };
    assert_eq!(
        reports,
        [
            (0, 0, ShellReport::Prompt),
            (2, 2, ShellReport::Ready),
            (5, 5, started("ls -la")),
            (8, 17, ShellReport::Finished { exit_code: Some(3) }),
            (8, 17, started("echo hi")),
            (8, 17, ShellReport::Finished { exit_code: None }),
            (8, 17, started("")),
            (8, 17, ShellReport::Finished { exit_code: None })
        ]
    );
    assert!(shell.take_shell_reports().is_empty());
}

#[test]
fn the_fingerprint_changes_with_the_screen_the_cursor_and_its_visibility() {
    let mut drawn = Screen::new(Size { cols: 5, rows: 2 });
    let blank = drawn.fingerprint();
    drawn.feed(b"a");
    let typed = drawn.fingerprint();
    drawn.feed(b"\x1b[1D");
    let moved = drawn.fingerprint();
    drawn.feed(b"\x1b[?25l");
    let hidden = drawn.fingerprint();
    drawn.feed(b"\x1b[31ma\x1b[1D");
    let coloured = drawn.fingerprint();
    drawn.feed(b"\x1b[0;44ma\x1b[1D");
    let backed = drawn.fingerprint();
    drawn.feed(b"\x1b[0;38;2;1;2;3ma\x1b[1D");
    let rgb = drawn.fingerprint();
    drawn.feed(b"\x1b[0;38;5;9ma\x1b[1D");
    let indexed = drawn.fingerprint();
    drawn.feed(b"\x1b[0;1ma\x1b[1D");
    let bold = drawn.fingerprint();
    drawn.feed(b"\x1b[0ma\x1b[1D");
    let plain_again = drawn.fingerprint();
    drawn.feed("e\u{301}\x1b[1D".as_bytes());
    let marked = drawn.fingerprint();
    let all = [blank, typed, moved, hidden, coloured, backed, rgb, indexed, bold, marked];
    let distinct: std::collections::BTreeSet<_> = all.iter().collect();
    assert_eq!(
        (distinct.len(), plain_again == hidden, Screen::new(Size { cols: 5, rows: 2 }).fingerprint() == blank),
        (10, true, true)
    );
}

#[test]
fn a_resized_screen_reports_its_new_size_and_rows_that_scrolled_off_are_kept() {
    let mut drawn = screen(5, 2, "1\r\n2\r\n3\r\n4");
    assert_eq!(
        (drawn.scrollback_len(), drawn.rows(-2..2, false)),
        (2, vec!["1".to_string(), "2".to_string(), "3".to_string(), "4".to_string()])
    );
    drawn.resize(Size { cols: 8, rows: 3 });
    assert_eq!((drawn.size(), drawn.view().lines.len(), drawn.cursor()), (Size { cols: 8, rows: 3 }, 3, (2, 1)));
}

#[test]
fn a_pattern_matches_rows_from_the_first_line_asked_with_one_based_columns() {
    let drawn = screen(12, 3, "b0\r\nxx 中b22\r\nb3 b4\r\n");
    let pattern = regex::Regex::new(r"b\d+|z*").unwrap();
    let found: Vec<_> =
        drawn.matches(&pattern, -1).into_iter().map(|found| (found.row, found.col, found.text)).collect();
    assert_eq!(
        found,
        [(-1, 1, "b0".to_string()), (1, 6, "b22".to_string()), (2, 1, "b3".to_string()), (2, 4, "b4".to_string())]
    );
    assert_eq!(drawn.matches(&pattern, 2).len(), 0);
}

#[test]
fn a_colour_reads_as_its_terminal_name_its_index_or_its_hex() {
    let named = |name: NamedColor| Colour::from(Color::Named(name));
    assert_eq!(
        [
            named(NamedColor::Red),
            named(NamedColor::BrightWhite),
            named(NamedColor::DimBlue),
            named(NamedColor::DimWhite),
            named(NamedColor::Foreground)
        ],
        [Colour::Indexed(1), Colour::Indexed(15), Colour::Indexed(4), Colour::Indexed(7), Colour::Default]
    );
    assert_eq!(
        (Colour::from(Color::Indexed(200)), Colour::from(Color::Spec(Rgb { r: 1, g: 2, b: 3 }))),
        (Colour::Indexed(200), Colour::Rgb(1, 2, 3))
    );
    let shown = [
        Colour::Default,
        Colour::Indexed(0),
        Colour::Indexed(8),
        Colour::Indexed(15),
        Colour::Indexed(16),
        Colour::Rgb(10, 11, 255),
    ]
    .map(|colour| colour.to_string());
    assert_eq!(shown, ["default", "black", "bright-black", "bright-white", "16", "#0a0bff"]);
}
