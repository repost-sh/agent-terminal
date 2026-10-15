use std::path::Path;
use std::time::Instant;

use serde_json::Value;

use crate::screenshot::picture;
use crate::support::{AgentTerminal, fixture};

struct Gif {
    width: u32,
    height: u32,
    delays_ms: Vec<u32>,
    last: Vec<u8>,
}

fn gif(path: &Path) -> Gif {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(file).unwrap_or_else(|e| panic!("{} is not a GIF: {e}", path.display()));
    let (width, height) = (u32::from(decoder.width()), u32::from(decoder.height()));
    let mut canvas = vec![0u8; (width * height * 3) as usize];
    let mut delays_ms = Vec::new();
    while let Some(frame) = decoder.read_next_frame().unwrap_or_else(|e| panic!("decode {}: {e}", path.display())) {
        delays_ms.push(u32::from(frame.delay) * 10);
        for (i, pixel) in frame.buffer.chunks_exact(4).enumerate() {
            if pixel[3] == 0 {
                continue;
            }
            let x = u32::from(frame.left) + i as u32 % u32::from(frame.width);
            let y = u32::from(frame.top) + i as u32 / u32::from(frame.width);
            let at = ((y * width + x) * 3) as usize;
            canvas[at..at + 3].copy_from_slice(&pixel[..3]);
        }
    }
    Gif { width, height, delays_ms, last: canvas }
}

#[track_caller]
fn assert_same_picture(width: u32, height: u32, rgb: &[u8], screenshot: &Path) {
    let expected = picture(screenshot);
    assert_eq!((width, height), (expected.width, expected.height), "the clip and the screenshot differ in size");
    let differing = rgb
        .chunks_exact(3)
        .zip(expected.rgb.chunks_exact(3))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a != b))
        .count();
    assert_eq!(differing, 0, "{differing} pixels of the last frame differ from {}", screenshot.display());
}

struct Mp4 {
    width: u32,
    height: u32,
    duration_ms: u64,
    durations_ms: Vec<u32>,
    last: Vec<u8>,
}

fn mp4(path: &Path) -> Mp4 {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let size = bytes.len() as u64;
    let mut reader = mp4::Mp4Reader::read_header(std::io::Cursor::new(bytes), size)
        .unwrap_or_else(|e| panic!("{} is not an MP4: {e}", path.display()));
    let track = reader.tracks().values().next().unwrap_or_else(|| panic!("{} has no track", path.display()));
    assert_eq!(track.media_type().unwrap(), mp4::MediaType::H264, "{} is not H.264", path.display());
    let (id, samples) = (track.track_id(), track.sample_count());
    let (width, height) = (u32::from(track.width()), u32::from(track.height()));
    let duration_ms = track.duration().as_millis() as u64;
    let mut parameters = Vec::new();
    for set in [track.sequence_parameter_set().unwrap(), track.picture_parameter_set().unwrap()] {
        parameters.extend_from_slice(&[0, 0, 0, 1]);
        parameters.extend_from_slice(set);
    }
    let mut decoder = openh264::decoder::Decoder::new().expect("an H.264 decoder");
    decoder.decode(&parameters).expect("the parameter sets decode");
    let mut last = vec![0u8; (width * height * 3) as usize];
    let mut durations_ms = Vec::new();
    for sample in 1..=samples {
        let sample = reader.read_sample(id, sample).unwrap().expect("a sample");
        durations_ms.push(sample.duration);
        let mut annex_b = Vec::new();
        let mut rest = &sample.bytes[..];
        while rest.len() >= 4 {
            let length = u32::from_be_bytes(rest[..4].try_into().unwrap()) as usize;
            annex_b.extend_from_slice(&[0, 0, 0, 1]);
            annex_b.extend_from_slice(&rest[4..4 + length]);
            rest = &rest[4 + length..];
        }
        if let Some(picture) = decoder.decode(&annex_b).expect("the sample decodes") {
            picture.write_rgb8(&mut last);
        }
    }
    Mp4 { width, height, duration_ms, durations_ms, last }
}

const LOSSY_CHANNEL_TOLERANCE: u8 = 24;
const LOSSY_PIXEL_TOLERANCE: f64 = 0.005;

#[track_caller]
fn assert_close_picture(width: u32, height: u32, rgb: &[u8], screenshot: &Path) {
    let expected = picture(screenshot);
    assert_eq!((width, height), (expected.width, expected.height), "the clip and the screenshot differ in size");
    let differing = rgb
        .chunks_exact(3)
        .zip(expected.rgb.chunks_exact(3))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > LOSSY_CHANNEL_TOLERANCE))
        .count();
    let share = differing as f64 / f64::from(width * height);
    assert!(
        share <= LOSSY_PIXEL_TOLERANCE,
        "{differing} pixels ({:.2}%) of the last frame differ from {}",
        share * 100.0,
        screenshot.display()
    );
}

#[test]
fn a_gif_of_the_last_second_of_a_still_screen_is_one_frame_a_second_long_ending_on_the_screen() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    at.run(&["open", "--size", "20x4", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    at.run(&["wait", "--idle", "1000"]).success();
    let screenshot = work.join("now.png");
    at.run(&["screenshot", screenshot.to_str().unwrap()]).success();
    let path = work.join("still.gif");

    let clip = at.run(&["--json", "clip", path.to_str().unwrap(), "--last", "1s"]).success().json();

    assert_eq!(clip["path"], path.to_str().unwrap());
    assert_eq!(clip["format"], "gif");
    assert_eq!(clip["frames"], 1);
    assert_eq!(clip["duration_ms"], 1000);
    let gif = gif(&path);
    assert_eq!(gif.delays_ms, [1000]);
    assert_same_picture(gif.width, gif.height, &gif.last, &screenshot);
}

#[test]
fn clips_land_in_the_screenshot_dir_by_default_and_the_activity_log_links_them() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    at.run(&["open", "--size", "20x4", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    at.run(&["wait", "--idle", "1000"]).success();
    let frame = at.run(&["get", "frame"]).success().stdout().trim().to_string();
    let name = format!("default-t1-{frame}.gif");

    let by_default = at.run(&["clip", "--last", "1s"]).success();
    let by_option = at.run_in(&work, &["--screenshot-dir", "shots", "clip", "--last", "1s"]).success();

    let default_path = at.home().join("screenshots").join(&name);
    let option_path = work.join("shots").join(&name);
    assert_eq!(by_default.stdout(), format!("saved {} (default t1, 1 frame, 1.0 s)\n", default_path.display()));
    assert_eq!(by_option.stdout(), format!("saved {} (default t1, 1 frame, 1.0 s)\n", option_path.display()));
    assert_eq!(gif(&default_path).delays_ms, [1000]);
    assert_eq!(gif(&option_path).delays_ms, [1000]);
    let log = at.activity("default");
    let clips: Vec<&Value> = log.iter().rev().take(2).rev().map(|entry| &entry["clips"]).collect();
    assert_eq!(
        clips,
        [&serde_json::json!([default_path.to_str().unwrap()]), &serde_json::json!([option_path.to_str().unwrap()])]
    );
    assert!(log.iter().take(log.len() - 2).all(|entry| entry.get("clips").is_none()), "{log:?}");
}

#[test]
fn a_screen_that_changes_all_the_time_makes_at_most_ten_frames_a_second() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    let counter = "i=0; while :; do i=$((i+1)); printf '\\rtick %d' $i; sleep 0.01; done";
    at.run(&["open", "--size", "20x4", "--wait", "tick", "sh", "-c", counter]).success();
    at.run(&["wait", "--frame", "100"]).success();
    let path = work.join("busy.gif");

    let clip = at.run(&["--json", "clip", path.to_str().unwrap(), "--last", "1s"]).success().json();

    let frames = clip["frames"].as_u64().unwrap();
    assert!((2..=10).contains(&frames), "{frames} frames in one second of a counter that ticks every 10 ms");
    let gif = gif(&path);
    assert_eq!(gif.delays_ms.len() as u64, frames);
    assert!(gif.delays_ms.iter().all(|delay| *delay >= 100), "a frame shorter than 1/10 s: {:?}", gif.delays_ms);
    assert_eq!(gif.delays_ms.iter().sum::<u32>(), 1000);
}

#[test]
fn a_clip_across_a_resize_is_as_large_as_the_largest_screen_and_ends_on_the_last_one() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    at.run(&["open", "--size", "20x4", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    at.run(&["mark", "small"]).success();
    at.run(&["resize", "30x6"]).success();
    let screenshot = work.join("large.png");
    at.run(&["screenshot", screenshot.to_str().unwrap()]).success();
    let path = work.join("resize.gif");

    let clip = at.run(&["--json", "clip", path.to_str().unwrap(), "--from", "small"]).success().json();

    let large = picture(&screenshot);
    assert_eq!((clip["width"].as_u64().unwrap(), clip["height"].as_u64().unwrap()), (290, 140));
    assert_eq!((large.width, large.height), (290, 140));
    let gif = gif(&path);
    assert_same_picture(gif.width, gif.height, &gif.last, &screenshot);
}

fn three_lines_after_enter(at: &AgentTerminal) {
    let script = format!(
        "stty -echo; printf 'ready\\n'; read _; sleep 0.3; {} --lines 3 --delay-ms 300 --hold",
        fixture("line-printer")
    );
    at.run(&["open", "--size", "20x6", "--wait", "ready", "sh", "-c", &script]).success();
}

#[test]
fn a_gif_between_two_marks_has_one_frame_per_screen_and_lasts_from_mark_to_mark() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    three_lines_after_enter(&at);
    let before_start = Instant::now();
    at.run(&["mark", "start"]).success();
    let after_start = Instant::now();
    at.run(&["press", "Enter", "--wait", "line 3"]).success();
    let before_end = Instant::now();
    at.run(&["mark", "end"]).success();
    let after_end = Instant::now();
    let screenshot = work.join("end.png");
    at.run(&["screenshot", screenshot.to_str().unwrap()]).success();
    let path = work.join("marks.gif");

    let clip = at.run(&["--json", "clip", path.to_str().unwrap(), "--from", "start", "--to", "end"]).success().json();

    assert_eq!(clip["frames"], 4, "ready, then line 1, line 2 and line 3 one by one: {clip}");
    let duration_ms = clip["duration_ms"].as_u64().unwrap();
    let shortest = before_end.duration_since(after_start).as_millis() as u64;
    let longest = after_end.duration_since(before_start).as_millis() as u64;
    assert!((shortest..=longest).contains(&duration_ms), "{duration_ms} ms is not between {shortest} and {longest}");
    let gif = gif(&path);
    assert_eq!(gif.delays_ms.len(), 4);
    assert!(gif.delays_ms.iter().all(|delay| *delay >= 100), "a frame shorter than 1/10 s: {:?}", gif.delays_ms);
    assert_eq!(u64::from(gif.delays_ms.iter().sum::<u32>()), (duration_ms + 5) / 10 * 10);
    assert_same_picture(gif.width, gif.height, &gif.last, &screenshot);
}

#[test]
fn an_mp4_between_two_marks_has_one_frame_per_screen_and_lasts_from_mark_to_mark() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    three_lines_after_enter(&at);
    at.run(&["mark", "start"]).success();
    at.run(&["press", "Enter", "--wait", "line 3"]).success();
    at.run(&["mark", "end"]).success();
    let screenshot = work.join("end.png");
    at.run(&["screenshot", screenshot.to_str().unwrap()]).success();
    let path = work.join("marks.mp4");

    let clip = at.run(&["--json", "clip", path.to_str().unwrap(), "--from", "start", "--to", "end"]).success().json();

    assert_eq!(clip["format"], "mp4");
    assert_eq!(clip["frames"], 4, "ready, then line 1, line 2 and line 3 one by one: {clip}");
    let video = mp4(&path);
    assert_eq!(video.durations_ms.len(), 4);
    assert!(video.durations_ms.iter().all(|duration| *duration >= 100), "{:?}", video.durations_ms);
    assert_eq!(u64::from(video.durations_ms.iter().sum::<u32>()), clip["duration_ms"].as_u64().unwrap());
    assert_eq!(video.duration_ms, clip["duration_ms"].as_u64().unwrap());
    assert_close_picture(video.width, video.height, &video.last, &screenshot);
}

#[test]
fn an_mp4_of_a_screen_an_odd_number_of_pixels_wide_gets_one_more_column_of_the_edge() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    at.run(&["open", "--size", "21x4", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    at.run(&["wait", "--idle", "1000"]).success();
    let screenshot = work.join("now.png");
    at.run(&["screenshot", screenshot.to_str().unwrap()]).success();
    let path = work.join("odd.mp4");

    let clip = at.run(&["--json", "clip", path.to_str().unwrap(), "--last", "1s"]).success().json();

    let shot = picture(&screenshot);
    assert_eq!((shot.width, shot.height), (209, 100));
    assert_eq!((clip["width"].as_u64().unwrap(), clip["height"].as_u64().unwrap()), (210, 100));
    let video = mp4(&path);
    assert_eq!((video.width, video.height, video.durations_ms.clone()), (210, 100, vec![1000]));
    let cropped: Vec<u8> = video.last.chunks_exact(210 * 3).flat_map(|row| row[..209 * 3].to_vec()).collect();
    assert_close_picture(209, 100, &cropped, &screenshot);
}

#[test]
fn a_gif_of_one_activity_entry_lasts_as_long_as_the_call_and_shows_what_it_did() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    three_lines_after_enter(&at);
    at.run(&["press", "Enter", "--wait", "line 3"]).success();
    let screenshot = work.join("end.png");
    at.run(&["screenshot", screenshot.to_str().unwrap()]).success();
    let pressed = &at.activity("default")[1];
    assert_eq!(pressed["command"], "press Enter --wait 'line 3'");
    let path = work.join("press.gif");

    let clip = at
        .run(&["--json", "clip", path.to_str().unwrap(), "--from", "@activity:2", "--to", "@activity:2"])
        .success()
        .json();

    assert_eq!(clip["frames"], 4, "ready, then line 1, line 2 and line 3 one by one: {clip}");
    let (took, lasted) = (pressed["duration_ms"].as_u64().unwrap(), clip["duration_ms"].as_u64().unwrap());
    assert!((took..=took + 2).contains(&lasted), "a {took} ms call, logged to the ms, made a {lasted} ms clip");
    let gif = gif(&path);
    assert_eq!(gif.delays_ms.len(), 4);
    assert_same_picture(gif.width, gif.height, &gif.last, &screenshot);
}

#[test]
fn a_clip_from_an_activity_entry_that_does_not_exist_says_how_many_there_are() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x4", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    at.run(&["get", "frame"]).success();

    let missing = at.run(&["clip", "--from", "@activity:3"]);
    let malformed = at.run(&["clip", "--from", "@activity:first"]);

    assert_eq!(missing.code(), 1);
    assert_eq!(
        missing.stderr(),
        "error: @activity:3 is not in the activity log of session default, which has 2 entries\n"
    );
    assert_eq!(malformed.code(), 1);
    assert!(
        malformed.stderr().contains("@activity:first is not an activity entry; count them from 1, e.g. @activity:3"),
        "{}",
        malformed.stderr()
    );
}

#[test]
fn a_gif_of_one_shell_command_runs_from_its_start_to_its_end() {
    let at = AgentTerminal::new();
    let work = at.work_folder();
    let home = work.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let run = |args: &[&str]| at.run_with_env(args, &[("HOME", &home)]).success();
    run(&["shell", "--shell", "bash", "--size", "40x8", "--cwd", work.to_str().unwrap()]);
    run(&["exec", "PS1="]);
    run(&["exec", "sleep 0.3; echo one; sleep 0.3; echo two; sleep 0.3"]);
    let screenshot = work.join("end.png");
    run(&["screenshot", screenshot.to_str().unwrap()]);
    let command = run(&["--json", "history", "show", "2"]).json();
    let path = work.join("command.gif");

    let clip = run(&["--json", "clip", path.to_str().unwrap(), "--command", "2"]).json();

    assert_eq!(clip["frames"], 3, "the command line, then one, then two: {clip}");
    let (took, lasted) = (command["duration_ms"].as_u64().unwrap(), clip["duration_ms"].as_u64().unwrap());
    assert!((took..=took + 2).contains(&lasted), "a {took} ms command, logged to the ms, made a {lasted} ms clip");
    let gif = gif(&path);
    assert_eq!(gif.delays_ms.len(), 3);
    assert_same_picture(gif.width, gif.height, &gif.last, &screenshot);
}

#[test]
fn a_clip_of_a_command_needs_a_shell_tab() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x4", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();

    let run = at.run(&["clip", "--command", "1"]);

    assert_eq!(run.code(), 1);
    assert!(
        run.stderr().starts_with(
            "error: default t1 runs line-printer without shell integration, so it keeps no command history;"
        ),
        "{}",
        run.stderr()
    );
}

#[test]
fn a_clip_from_a_mark_nobody_set_fails_and_says_how_to_set_it() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x4", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();

    let run = at.run(&["clip", "--from", "nowhere"]);

    assert_eq!(run.code(), 1);
    assert_eq!(
        run.stderr(),
        "error: no mark named \"nowhere\" in default t1; name a point with `agent-terminal mark nowhere`\n"
    );
}

#[test]
fn a_clip_is_a_gif_or_an_mp4_and_nothing_else() {
    let at = AgentTerminal::new();

    let run = at.run(&["clip", "evidence/crash.png"]);

    assert_eq!(run.code(), 1);
    assert_eq!(run.stderr(), "error: a clip is a .gif or .mp4 file; evidence/crash.png is neither\n");
}
