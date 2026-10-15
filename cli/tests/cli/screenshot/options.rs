use std::path::{Path, PathBuf};

use super::picture::{pixel, png_size};
use super::screen::Screen;
use crate::support::{AgentTerminal, fixture};

const DARK_BACKGROUND: [u8; 3] = [0x11, 0x11, 0x13];
const DARK_FOREGROUND: [u8; 3] = [0xd4, 0xd4, 0xd8];
const LIGHT_BACKGROUND: [u8; 3] = [0xfa, 0xfa, 0xfa];

fn shot(at: &AgentTerminal, work: &Path, name: &str, options: &[&str]) -> PathBuf {
    let path = work.join(name);
    let mut args = vec!["screenshot", path.to_str().unwrap()];
    args.extend_from_slice(options);
    at.run(&args).success();
    path
}

#[test]
fn scale_rows_theme_and_cursor_options_change_the_picture() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x4", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    at.run(&["wait", "--cursor", "4,1"]).success();
    let work = at.work_folder();

    scale_and_rows_change_the_size(&at, &work);
    theme_and_cursor_change_the_pixels(&at, &work);
}

fn scale_and_rows_change_the_size(at: &AgentTerminal, work: &Path) {
    assert_eq!(png_size(&shot(at, work, "double.png", &["--scale", "2"])), (400, 200));
    assert_eq!(png_size(&shot(at, work, "rows.png", &["--rows", "2:3"])), (200, 60));
    let outside = at.run(&["screenshot", "--rows", "5:9"]);
    assert_eq!((outside.code(), outside.stderr()), (1, "error: rows 5:9 are outside the 20x4 screen\n".to_string()));
}

fn theme_and_cursor_change_the_pixels(at: &AgentTerminal, work: &Path) {
    let dark = shot(at, work, "dark.png", &[]);
    assert_eq!(pixel(&dark, 0, 0), DARK_BACKGROUND);
    assert_eq!(pixel(&shot(at, work, "light.png", &["--theme", "light"]), 0, 0), LIGHT_BACKGROUND);

    let in_cursor_cell = (10 + 4, 10 + 3 * 20 + 10);
    assert_eq!(pixel(&dark, in_cursor_cell.0, in_cursor_cell.1), DARK_FOREGROUND);
    let no_cursor = shot(at, work, "no-cursor.png", &["--no-cursor"]);
    assert_eq!(pixel(&no_cursor, in_cursor_cell.0, in_cursor_cell.1), DARK_BACKGROUND);
}

fn file(screen: &Screen, name: &str) -> String {
    screen.file(name).to_string_lossy().into_owned()
}

fn read(screen: &Screen, name: &str) -> String {
    std::fs::read_to_string(screen.file(name)).unwrap()
}

#[test]
fn other_formats_follow_the_extension_or_the_format_option() {
    let at = AgentTerminal::new();
    let screen = Screen::open(&at, "20x3", "\x1b[?25l\x1b[31mred\x1b[0m plain\nsecond", "second");

    at.run(&["screenshot", &file(&screen, "screen.txt")]).success();
    at.run(&["screenshot", "--format", "ansi", &file(&screen, "screen.ans")]).success();
    at.run(&["screenshot", &file(&screen, "screen.html")]).success();
    let svg = at.run(&["--json", "screenshot", &file(&screen, "screen.svg")]).success().json();

    assert_eq!(read(&screen, "screen.txt"), "red plain\nsecond\n\n");
    assert_eq!(read(&screen, "screen.ans"), "\x1b[0;31mred\x1b[0m plain\nsecond\n\n");
    the_html_file_is_a_page_with_the_coloured_text(&screen);
    the_svg_file_draws_the_coloured_text(&screen, &svg);

    let refused = at.run(&["screenshot", "--annotate", &file(&screen, "refs.txt")]);
    assert_eq!(refused.code(), 1);
    assert_eq!(refused.stderr(), "error: --annotate draws on an image, so it needs png or svg, not txt\n");
}

fn the_html_file_is_a_page_with_the_coloured_text(screen: &Screen) {
    let html = read(screen, "screen.html");
    assert!(html.starts_with("<!DOCTYPE html>"), "{html}");
    assert!(html.contains("<pre><span style=\"color:#f87171\">red</span> plain\nsecond\n\n</pre>"), "{html}");
}

fn the_svg_file_draws_the_coloured_text(screen: &Screen, svg: &serde_json::Value) {
    let drawing = read(screen, "screen.svg");
    assert!(drawing.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"200\" height=\"80\""), "{drawing}");
    assert!(drawing.contains("fill=\"#f87171\">red</text>"), "{drawing}");
    assert_eq!(
        (svg["format"].clone(), svg["width"].clone(), svg["height"].clone()),
        ("svg".into(), 200.into(), 80.into())
    );
}
