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

const CHANNEL_TOLERANCE: u8 = 24;
const PIXEL_TOLERANCE: f64 = 0.005;

#[track_caller]
fn assert_looks_like(actual: &Path, baseline: &str) {
    let expected_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/baselines").join(baseline);
    let kept = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/baseline-failures").join(baseline);
    let keep = || {
        std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
        std::fs::copy(actual, &kept).unwrap();
    };
    if !expected_path.exists() {
        keep();
        panic!("no baseline {}; the screenshot is in {}", expected_path.display(), kept.display());
    }
    let (got, expected) = (picture(actual), picture(&expected_path));
    if (got.width, got.height) != (expected.width, expected.height) {
        keep();
        panic!(
            "{} is {}x{} px, the baseline {}x{} px; the screenshot is in {}",
            baseline,
            got.width,
            got.height,
            expected.width,
            expected.height,
            kept.display()
        );
    }
    let differing = got
        .rgb
        .chunks_exact(3)
        .zip(expected.rgb.chunks_exact(3))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > CHANNEL_TOLERANCE))
        .count();
    let share = differing as f64 / (got.width * got.height) as f64;
    if share > PIXEL_TOLERANCE {
        keep();
        panic!(
            "{differing} pixels ({:.2}%) differ from the baseline {baseline}; the screenshot is in {}",
            share * 100.0,
            kept.display()
        );
    }
}

struct Screen {
    dir: tempfile::TempDir,
}

impl Screen {
    fn open(at: &AgentTerminal, size: &str, bytes: &str, wait: &str) -> Self {
        let dir = tempfile::Builder::new().prefix("screen-").tempdir_in("/tmp").expect("temp screen folder");
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
fn a_screenshot_draws_box_drawing_wide_characters_emoji_and_colors_like_the_baseline() {
    let at = AgentTerminal::new();
    let screen = Screen::open(&at, "44x8", EVERYTHING, "end");
    let path = screen.file("everything.png");

    at.run(&["screenshot", &path.to_string_lossy()]).success();

    assert_looks_like(&path, "everything.png");
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

fn work_folder(at: &AgentTerminal) -> PathBuf {
    let work = at.home().join("work");
    std::fs::create_dir_all(&work).unwrap();
    work.canonicalize().unwrap()
}

#[test]
fn screenshots_go_to_the_screenshot_dir_its_variable_or_the_path_given() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x4", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    let frame = frame(&at);
    let work = work_folder(&at);
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
    let work = work_folder(&at);
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
    let work = work_folder(&at);
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
    assert_looks_like(&path, "annotated.png");
}

#[test]
fn diff_screenshot_passes_when_the_screen_still_looks_like_the_baseline() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui")]).success();
    let work = work_folder(&at);
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
    let work = work_folder(&at);
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
    let work = work_folder(&at);
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
