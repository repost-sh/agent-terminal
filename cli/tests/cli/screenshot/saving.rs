use super::picture::png_size;
use crate::support::{AgentTerminal, fixture};

#[test]
fn screenshot_saves_a_png_of_the_screen_named_after_session_tab_and_frame() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x4", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    let frame = at.frame("default");

    let run = at.run(&["screenshot"]).success();

    let path = at.home().join("screenshots").join(format!("default-t1-{frame}.png"));
    assert_eq!(run.stdout(), format!("saved {} (default t1, frame {frame})\n", path.display()));
    assert_eq!(png_size(&path), (200, 100));
}

#[test]
fn screenshots_of_the_same_frame_taken_at_once_all_succeed_and_leave_one_whole_file() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x4", "--wait", "line 3", &fixture("line-printer"), "--hold"]).success();
    let frame = at.frame("default");

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
    let frame = at.frame("default");
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

#[test]
fn an_input_command_saves_a_screenshot_once_its_wait_is_over() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui")]).success();
    let work = at.work_folder();
    let given = work.join("beta.png");

    let pressed = at.run(&["press", "j", "--wait", "Selected: beta.txt", "--screenshot", given.to_str().unwrap()]);
    let beta = at.frame("default");
    let json = at.run(&["--json", "press", "k", "--wait", "Selected: alpha.txt", "--screenshot"]).success().json();
    let alpha = at.frame("default");

    let expected = format!("pressed 1 key in default t1\nsaved {} (default t1, frame {beta})\n", given.display());
    assert_eq!(pressed.success().stdout(), expected);
    assert_eq!(png_size(&given), (470, 180));
    let default = at.home().join("screenshots").join(format!("default-t1-{alpha}.png"));
    assert_eq!(json["screenshot"]["path"], default.to_string_lossy().as_ref());
    assert_eq!(json["screenshot"]["frame"], alpha);
    assert_eq!(png_size(&default), (470, 180));
}
