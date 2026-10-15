use std::path::{Path, PathBuf};

use crate::support::{AgentTerminal, fixture};

pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

pub fn picture(path: &Path) -> Picture {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().unwrap_or_else(|e| panic!("{} is not a PNG: {e}", path.display()));
    let mut pixels = vec![0; reader.output_buffer_size().expect("a decodable size")];
    let frame = reader.next_frame(&mut pixels).unwrap_or_else(|e| panic!("decode {}: {e}", path.display()));
    let channels = frame.color_type.samples();
    let rgb = pixels[..frame.buffer_size()].chunks_exact(channels).flat_map(|pixel| [pixel[0], pixel[1], pixel[2]]);
    Picture { width: frame.width, height: frame.height, rgb: rgb.collect() }
}

fn png_size(path: &Path) -> (u32, u32) {
    let picture = picture(path);
    (picture.width, picture.height)
}

const CELL_WIDTH: u32 = 9;
const CELL_HEIGHT: u32 = 20;
const MARGIN: u32 = 10;
const CLOSE: u8 = 24;

type Rgb = [u8; 3];

const BACKGROUND: Rgb = [0x11, 0x11, 0x13];
const FOREGROUND: Rgb = [0xd4, 0xd4, 0xd8];
const RED: Rgb = [0xf8, 0x71, 0x71];
const GREEN: Rgb = [0x4a, 0xde, 0x80];
const BLUE: Rgb = [0x60, 0xa5, 0xfa];
const XTERM_208: Rgb = [255, 135, 0];
const MAGENTA_RGB: Rgb = [255, 0, 255];
const REF_COLORS: [Rgb; 6] = [
    [0xfb, 0x92, 0x3c],
    [0x38, 0xbd, 0xf8],
    [0xa3, 0xe6, 0x35],
    [0xf4, 0x72, 0xb6],
    [0xfa, 0xcc, 0x15],
    [0x81, 0x8c, 0xf8],
];

fn close_to(a: Rgb, b: Rgb) -> bool {
    a.iter().zip(b).all(|(a, b)| a.abs_diff(b) <= CLOSE)
}

impl Picture {
    fn at(&self, x: u32, y: u32) -> Rgb {
        assert!(x < self.width && y < self.height, "{x},{y} is outside the {}x{} picture", self.width, self.height);
        let at = ((y * self.width + x) * 3) as usize;
        [self.rgb[at], self.rgb[at + 1], self.rgb[at + 2]]
    }

    fn cell(&self, row: u32, col: u32) -> Cell<'_> {
        Cell { picture: self, left: MARGIN + (col - 1) * CELL_WIDTH, top: MARGIN + (row - 1) * CELL_HEIGHT }
    }
}

struct Cell<'a> {
    picture: &'a Picture,
    left: u32,
    top: u32,
}

impl Cell<'_> {
    fn pixels(&self, xs: std::ops::Range<u32>, ys: std::ops::Range<u32>) -> Vec<Rgb> {
        ys.flat_map(|y| xs.clone().map(move |x| (x, y)))
            .map(|(x, y)| self.picture.at(self.left + x, self.top + y))
            .collect()
    }

    fn all(&self) -> Vec<Rgb> {
        self.pixels(0..CELL_WIDTH, 0..CELL_HEIGHT)
    }

    fn inked(&self, xs: std::ops::Range<u32>, ys: std::ops::Range<u32>, background: Rgb) -> bool {
        self.pixels(xs, ys).into_iter().any(|pixel| !close_to(pixel, background))
    }

    fn has(&self, color: Rgb) -> bool {
        self.all().into_iter().any(|pixel| close_to(pixel, color))
    }

    fn brightness(&self) -> u32 {
        self.all().iter().map(|pixel| pixel.iter().map(|&channel| u32::from(channel)).sum::<u32>()).sum()
    }
}

#[track_caller]
fn assert_filled(cell: &Cell, xs: std::ops::Range<u32>, ys: std::ops::Range<u32>, color: Rgb, what: &str) {
    let off: Vec<Rgb> = cell.pixels(xs, ys).into_iter().filter(|pixel| !close_to(*pixel, color)).collect();
    assert!(off.is_empty(), "{what}: {} pixels are not {color:?}, e.g. {:?}", off.len(), off.first());
}

#[track_caller]
fn assert_lines(cell: &Cell, ends: [bool; 4], what: &str) {
    let (mid_x, mid_y) = (CELL_WIDTH / 2, CELL_HEIGHT / 2);
    let edges = [
        ("top", mid_x - 1..mid_x + 2, 0..1),
        ("right", CELL_WIDTH - 1..CELL_WIDTH, mid_y - 2..mid_y + 3),
        ("bottom", mid_x - 1..mid_x + 2, CELL_HEIGHT - 1..CELL_HEIGHT),
        ("left", 0..1, mid_y - 2..mid_y + 3),
    ];
    for ((edge, xs, ys), reaches) in edges.into_iter().zip(ends) {
        assert_eq!(cell.inked(xs, ys, BACKGROUND), reaches, "{what}: a line reaches its {edge} edge");
    }
}

struct Screen {
    dir: tempfile::TempDir,
}

impl Screen {
    fn open(at: &AgentTerminal, size: &str, bytes: &str, wait: &str) -> Self {
        let dir = crate::support::scratch("screen-");
        let path = dir.path().join("screen");
        std::fs::write(&path, bytes).expect("write the screen");
        let path = path.to_string_lossy().into_owned();
        at.run(&["open", "--size", size, "--wait", wait, "sh", "-c", "cat \"$1\"; read _", "sh", &path]).success();
        Self { dir }
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
}

const EVERYTHING: &str = "\x1b[?25l\
┌─ Files ──┐ ╭─ Log ──╮ ╔═ Help ═╗\n\
│ \x1b[7malpha.txt\x1b[0m│ │ ok     │ ║ q quit ║\n\
├──────────┤ ╰────────╯ ╠════════╣\n\
└──────────┘ ┏━━┓ ┃x┃   ╚════════╝\n\
\x1b[31mred\x1b[0m \x1b[32mgreen\x1b[0m \x1b[1;33mbold\x1b[0m \x1b[44;37m blue \x1b[0m \x1b[38;5;208m256\x1b[0m \x1b[38;2;255;0;255mrgb\x1b[0m \x1b[3mitalic\x1b[0m \x1b[4munder\x1b[0m\n\
漢字 wide · 😀 🚀 emoji · \u{f07b} \u{e0b0} nerd\n\
▁▂▃▄▅▆▇█ ░▒▓ ▖▗▘▝ ┼╋╬ ╱╲ end";

fn frame(at: &AgentTerminal) -> u64 {
    at.run(&["get", "frame"]).success().stdout().trim().parse().expect("a frame number")
}

#[test]
fn a_screenshot_draws_colors_box_drawing_blocks_emoji_and_symbols_in_their_cells() {
    let at = AgentTerminal::new();
    let screen = Screen::open(&at, "44x8", EVERYTHING, "end");
    let path = screen.file("everything.png");

    at.run(&["screenshot", &path.to_string_lossy()]).success();

    let shot = picture(&path);
    assert_eq!((shot.width, shot.height), (44 * 9 + 2 * 10, 8 * 20 + 2 * 10));
    assert_eq!(shot.at(0, 0), BACKGROUND, "the margin");
    assert_filled(&shot.cell(8, 44), 0..9, 0..20, BACKGROUND, "an empty cell");
    assert_filled(&shot.cell(7, 29), 0..9, 0..20, BACKGROUND, "the hidden cursor's cell");

    for col in 3..=11 {
        let inverse = shot.cell(2, col);
        assert_filled(&inverse, 0..9, 0..2, FOREGROUND, "the inverse row's backdrop");
        assert!(inverse.has(BACKGROUND), "the inverse text at col {col} is drawn in the background color");
    }
    assert!(shot.cell(5, 1).has(RED), "\"red\" is red");
    assert!(shot.cell(5, 5).has(GREEN), "\"green\" is green");
    assert_filled(&shot.cell(5, 16), 0..9, 0..20, BLUE, "the space before \"blue\" on a blue background");
    assert!(shot.cell(5, 23).has(XTERM_208), "\"256\" is xterm color 208");
    assert!(shot.cell(5, 27).has(MAGENTA_RGB), "\"rgb\" is #ff00ff");

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

    assert_filled(&shot.cell(7, 8), 0..9, 0..20, FOREGROUND, "█ fills its cell");
    assert_filled(&shot.cell(7, 4), 0..9, 0..9, BACKGROUND, "▄ leaves the upper half");
    assert_filled(&shot.cell(7, 4), 0..9, 11..20, FOREGROUND, "▄ fills the lower half");
    assert_filled(&shot.cell(7, 1), 0..9, 0..17, BACKGROUND, "▁ leaves all but the lowest eighth");
    assert_filled(&shot.cell(7, 1), 0..9, 19..20, FOREGROUND, "▁ fills the lowest eighth");
    let quadrants = [("▖", 14, [false, false, true, false]), ("▗", 15, [false, false, false, true])];
    let quadrants =
        quadrants.into_iter().chain([("▘", 16, [true, false, false, false]), ("▝", 17, [false, true, false, false])]);
    for (block, col, filled) in quadrants {
        let cell = shot.cell(7, col);
        let parts = [(0..4, 0..9), (5..9, 0..9), (0..4, 11..20), (5..9, 11..20)];
        for ((xs, ys), on) in parts.into_iter().zip(filled) {
            let color = if on { FOREGROUND } else { BACKGROUND };
            assert_filled(&cell, xs, ys, color, block);
        }
    }
    let shades: Vec<u32> = (10..=12).map(|col| shot.cell(7, col).brightness()).collect();
    assert!(shades[0] < shades[1] && shades[1] < shades[2], "░ ▒ ▓ get darker to lighter: {shades:?}");

    for col in [13, 14, 16, 17] {
        assert!(shot.cell(6, col).inked(0..9, 0..20, BACKGROUND), "the emoji take two cells each: col {col}");
    }
    let arrow = shot.cell(6, 29);
    assert!(arrow.inked(0..2, 9..11, BACKGROUND), "the powerline arrow  starts at its cell's left edge");
    assert!(arrow.inked(3..6, 9..11, BACKGROUND), "the powerline arrow  is solid, not an empty box");
    assert!(!arrow.inked(7..9, 0..2, BACKGROUND), "the powerline arrow  narrows to a point");
    assert!(shot.cell(6, 27).inked(0..9, 0..20, BACKGROUND), "the nerd font folder  is drawn");
}

#[test]
fn screenshot_saves_a_png_of_the_screen_named_after_session_tab_and_frame() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x4", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    let frame = frame(&at);

    let run = at.run(&["screenshot"]).success();

    let path = at.home().join("screenshots").join(format!("default-t1-{frame}.png"));
    assert_eq!(run.stdout(), format!("saved {} (default t1, frame {frame})\n", path.display()));
    assert_eq!(png_size(&path), (200, 100));
}

#[test]
fn screenshots_of_the_same_frame_taken_at_once_all_succeed_and_leave_one_whole_file() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x4", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    let frame = frame(&at);

    let codes: Vec<i32> = std::thread::scope(|scope| {
        let shots: Vec<_> = (0..6).map(|_| scope.spawn(|| at.run(&["screenshot"]).code())).collect();
        shots.into_iter().map(|shot| shot.join().unwrap()).collect()
    });

    assert_eq!(codes, vec![0; 6]);
    let folder = at.home().join("screenshots");
    let files: Vec<String> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(files, vec![format!("default-t1-{frame}.png")]);
    assert_eq!(png_size(&folder.join(&files[0])), (200, 100));
}

#[test]
fn screenshots_go_to_the_screenshot_dir_its_variable_or_the_path_given() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x4", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    let frame = frame(&at);
    let work = at.work_folder();
    let name = format!("default-t1-{frame}.png");

    let by_option = at.run_in(&work, &["--screenshot-dir", "shots", "screenshot"]).success();
    let from_variable = at
        .run_with(&["screenshot"], "", &[("AGENT_TERMINAL_SCREENSHOT_DIR", &work.join("env").to_string_lossy())])
        .success();
    let given = at.run_in(&work, &["--screenshot-dir", "shots", "screenshot", "evidence/first.png"]).success();

    for (run, path) in [
        (by_option, work.join("shots").join(&name)),
        (from_variable, work.join("env").join(&name)),
        (given, work.join("evidence").join("first.png")),
    ] {
        assert_eq!(run.stdout(), format!("saved {} (default t1, frame {frame})\n", path.display()));
        assert_eq!(png_size(&path), (200, 100));
    }
}

fn pixel(path: &Path, x: u32, y: u32) -> [u8; 3] {
    let picture = picture(path);
    let at = ((y * picture.width + x) * 3) as usize;
    [picture.rgb[at], picture.rgb[at + 1], picture.rgb[at + 2]]
}

const DARK_BACKGROUND: [u8; 3] = [0x11, 0x11, 0x13];
const DARK_FOREGROUND: [u8; 3] = [0xd4, 0xd4, 0xd8];
const LIGHT_BACKGROUND: [u8; 3] = [0xfa, 0xfa, 0xfa];

#[test]
fn scale_rows_theme_and_cursor_options_change_the_picture() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x4", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    at.run(&["wait", "--cursor", "4,1"]).success();
    let work = at.work_folder();
    let shot = |name: &str, options: &[&str]| {
        let path = work.join(name);
        let mut args = vec!["screenshot", path.to_str().unwrap()];
        args.extend_from_slice(options);
        at.run(&args).success();
        path
    };

    assert_eq!(png_size(&shot("double.png", &["--scale", "2"])), (400, 200));
    assert_eq!(png_size(&shot("rows.png", &["--rows", "2:3"])), (200, 60));
    let outside = at.run(&["screenshot", "--rows", "5:9"]);
    assert_eq!((outside.code(), outside.stderr()), (1, "error: rows 5:9 are outside the 20x4 screen\n".to_string()));

    let dark = shot("dark.png", &[]);
    assert_eq!(pixel(&dark, 0, 0), DARK_BACKGROUND);
    assert_eq!(pixel(&shot("light.png", &["--theme", "light"]), 0, 0), LIGHT_BACKGROUND);

    let in_cursor_cell = (10 + 4, 10 + 3 * 20 + 10);
    assert_eq!(pixel(&dark, in_cursor_cell.0, in_cursor_cell.1), DARK_FOREGROUND);
    let no_cursor = shot("no-cursor.png", &["--no-cursor"]);
    assert_eq!(pixel(&no_cursor, in_cursor_cell.0, in_cursor_cell.1), DARK_BACKGROUND);
}

#[test]
fn other_formats_follow_the_extension_or_the_format_option() {
    let at = AgentTerminal::new();
    let screen = Screen::open(&at, "20x3", "\x1b[?25l\x1b[31mred\x1b[0m plain\nsecond", "second");
    let file = |name: &str| screen.file(name).to_string_lossy().into_owned();

    at.run(&["screenshot", &file("screen.txt")]).success();
    at.run(&["screenshot", "--format", "ansi", &file("screen.ans")]).success();
    at.run(&["screenshot", &file("screen.html")]).success();
    let svg = at.run(&["--json", "screenshot", &file("screen.svg")]).success().json();

    let read = |name: &str| std::fs::read_to_string(screen.file(name)).unwrap();
    assert_eq!(read("screen.txt"), "red plain\nsecond\n\n");
    assert_eq!(read("screen.ans"), "\x1b[0;31mred\x1b[0m plain\nsecond\n\n");
    let html = read("screen.html");
    assert!(html.starts_with("<!DOCTYPE html>"), "{html}");
    assert!(html.contains("<pre><span style=\"color:#f87171\">red</span> plain\nsecond\n\n</pre>"), "{html}");
    let drawing = read("screen.svg");
    assert!(drawing.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"200\" height=\"80\""), "{drawing}");
    assert!(drawing.contains("fill=\"#f87171\">red</text>"), "{drawing}");
    assert_eq!(
        (svg["format"].clone(), svg["width"].clone(), svg["height"].clone()),
        ("svg".into(), 200.into(), 80.into())
    );

    let refused = at.run(&["screenshot", "--annotate", &file("refs.txt")]);
    assert_eq!(refused.code(), 1);
    assert_eq!(refused.stderr(), "error: --annotate draws on an image, so it needs png or svg, not txt\n");
}

#[test]
fn annotate_draws_a_box_and_a_name_on_each_ref_with_a_legend_and_prints_the_refs() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui")]).success();
    let work = at.work_folder();
    let path = work.join("annotated.png");

    let run = at.run(&["screenshot", "--annotate", path.to_str().unwrap()]).success();

    insta::assert_snapshot!(run.screen().replace(&path.display().to_string(), "<path>"), @r#"
    saved <path> (default t1, frame N)
    refs:
      @t1 [panel]    "Files"     rows 1-7
      @t2 [panel]    "Details"   rows 1-7
      @t3 [selected] "alpha.txt" row 2 col 2
      @t4 [hint]     "Move: j/k" row 8 col 1
      @t5 [hint]     "Quit: q"   row 8 col 13
      @t6 [hint]     "Crash: x"  row 8 col 23
    "#);

    let shot = picture(&path);
    let legend_lines = 3;
    assert_eq!((shot.width, shot.height), (50 * 9 + 2 * 10, 8 * 20 + 2 * 10 + 10 + legend_lines * 20));
    let refs: [(u32, u32, u32, u32); 6] =
        [(1, 7, 1, 20), (1, 7, 21, 50), (2, 2, 2, 19), (8, 8, 1, 9), (8, 8, 13, 19), (8, 8, 23, 30)];
    for (index, ((first_row, last_row, first_col, last_col), color)) in refs.into_iter().zip(REF_COLORS).enumerate() {
        let name = format!("@t{}", index + 1);
        let (left, right) = (MARGIN + (first_col - 1) * CELL_WIDTH, MARGIN + last_col * CELL_WIDTH - 1);
        let (top, bottom) = (MARGIN + (first_row - 1) * CELL_HEIGHT, MARGIN + last_row * CELL_HEIGHT - 1);
        let (middle_x, middle_y) = ((left + right) / 2, (top + bottom) / 2);
        assert_eq!(shot.at(left, middle_y), color, "the left side of {name}'s box");
        assert_eq!(shot.at(right, middle_y), color, "the right side of {name}'s box");
        assert_eq!(shot.at(middle_x, bottom), color, "the bottom of {name}'s box");
    }
    let legend_top = 8 * 20 + 2 * 10;
    for (line, first_ref) in [0, 2, 4].into_iter().enumerate() {
        let y = legend_top + line as u32 * CELL_HEIGHT;
        assert_eq!(
            shot.at(MARGIN, y),
            REF_COLORS[first_ref],
            "legend line {} starts with @t{}",
            line + 1,
            first_ref + 1
        );
    }
}

#[test]
fn diff_screenshot_passes_when_the_screen_still_looks_like_the_baseline() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui")]).success();
    let work = at.work_folder();
    at.run_in(&work, &["screenshot", "before.png"]).success();
    let frame = frame(&at);

    let run = at.run_in(&work, &["diff", "screenshot", "--baseline", "before.png"]).success();

    let image = at.home().join("screenshots").join(format!("default-t1-{frame}-diff.png"));
    assert_eq!(
        run.stdout(),
        format!(
            "0 of 84600 pixels (0.00%) differ from {}, within the threshold 0; diff image {}\n",
            work.join("before.png").display(),
            image.display()
        )
    );
    assert_eq!(png_size(&image), (470, 180));
}

const CHANGED: [u8; 3] = [0xe6, 0x1e, 0x3c];

#[test]
fn diff_screenshot_highlights_what_changed_and_fails_above_the_threshold() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui")]).success();
    let work = at.work_folder();
    at.run_in(&work, &["screenshot", "before.png"]).success();
    at.run(&["press", "j", "--wait", "Selected: beta.txt"]).success();

    let failed = at.run_in(&work, &["diff", "screenshot", "--baseline", "before.png", "--output", "changes.png"]);
    let within = at.run_in(&work, &["--json", "diff", "screenshot", "--baseline", "before.png", "--threshold", "0.5"]);

    assert_eq!(failed.code(), 2, "{}{}", failed.stdout(), failed.stderr());
    let summary = failed.stdout();
    let changed: u64 = summary.split(' ').next().unwrap().parse().unwrap();
    assert!(changed > 100, "{summary}");
    assert!(
        summary.ends_with(&format!(
            "differ from {}, above the threshold 0; diff image {}\n",
            work.join("before.png").display(),
            work.join("changes.png").display()
        )),
        "{summary}"
    );
    let changes = work.join("changes.png");
    let beta_row = (10 + 10, 10 + 2 * 20 + 10);
    let untouched = (10 + 30 * 9 + 4, 10 + 6 * 20 + 10);
    assert_eq!(pixel(&changes, beta_row.0, beta_row.1), CHANGED);
    assert_ne!(pixel(&changes, untouched.0, untouched.1), CHANGED);
    let json = within.success().json();
    assert_eq!(json["changed"], changed);
    assert_eq!(json["pixels"], 84600);
    assert_eq!(json["threshold"], 0.5);
    assert_eq!(json["passed"], true);
}

#[test]
fn an_input_command_saves_a_screenshot_once_its_wait_is_over() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui")]).success();
    let work = at.work_folder();
    let given = work.join("beta.png");

    let pressed = at.run(&["press", "j", "--wait", "Selected: beta.txt", "--screenshot", given.to_str().unwrap()]);
    let beta = frame(&at);
    let json = at.run(&["--json", "press", "k", "--wait", "Selected: alpha.txt", "--screenshot"]).success().json();
    let alpha = frame(&at);

    let expected = format!("pressed 1 key in default t1\nsaved {} (default t1, frame {beta})\n", given.display());
    assert_eq!(pressed.success().stdout(), expected);
    assert_eq!(png_size(&given), (470, 180));
    let default = at.home().join("screenshots").join(format!("default-t1-{alpha}.png"));
    assert_eq!(json["screenshot"]["path"], default.to_string_lossy().as_ref());
    assert_eq!(json["screenshot"]["frame"], alpha);
    assert_eq!(png_size(&default), (470, 180));
}
