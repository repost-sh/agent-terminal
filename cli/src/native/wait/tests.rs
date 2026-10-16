use std::time::{Duration, Instant};

use super::{Condition, Moment, Pattern, Seen, Verdict, compile};
use crate::native::commands::PromptState;
use crate::native::screen::Screen;
use crate::protocol::{RowRange, Size, TextMatch, Wait};

const MS: Duration = Duration::from_millis(1);

struct Scene {
    screen: Screen,
    start: Instant,
    frame: u64,
    last_change: Instant,
    last_output: Instant,
    exited: bool,
    prompt: Option<PromptState>,
}

impl Scene {
    fn new(bytes: &str) -> Self {
        let mut screen = Screen::new(Size { cols: 20, rows: 3 });
        screen.feed(bytes.as_bytes());
        let start = Instant::now().checked_sub(MS * 100).unwrap();
        Self { screen, start, frame: 5, last_change: start, last_output: start, exited: false, prompt: None }
    }

    fn at(&self, ms: u32) -> Instant {
        self.start + MS * ms
    }

    fn judge(&self, condition: &Condition, deadline: Instant) -> String {
        let seen = Seen {
            screen: &self.screen,
            frame: self.frame,
            last_change: self.last_change,
            last_output: self.last_output,
            exited: self.exited,
            prompt: self.prompt,
        };
        match condition.judge(&Moment::new(seen), deadline) {
            Verdict::Met => "met".to_string(),
            Verdict::Never => "never".to_string(),
            Verdict::Pending(None) => "pending".to_string(),
            Verdict::Pending(Some(until)) => {
                format!("pending until {}ms", until.duration_since(self.start).as_millis())
            }
        }
    }

    fn verdict(&self, wait: &Wait) -> String {
        self.judge(&Condition::new(wait).unwrap(), self.at(100))
    }
}

fn text(text: &str, regex: bool, rows: Option<(u16, u16)>) -> TextMatch {
    TextMatch { text: text.to_string(), regex, rows: rows.map(|(first, last)| RowRange { first, last }) }
}

#[test]
fn a_pattern_is_plain_text_unless_asked_for_a_regex_and_finds_rows_counted_from_one() {
    let lines = ["a.b", "axb", "a.b again"].map(String::from);
    let rows = |matching: &TextMatch| Pattern::new(matching).unwrap().rows_found(&lines);
    assert_eq!(
        [rows(&text("a.b", false, None)), rows(&text("a.b", true, None)), rows(&text("a.b", true, Some((2, 3))))],
        [vec![1, 3], vec![1, 2, 3], vec![2, 3]]
    );
    assert!(Pattern::new(&text("x", false, Some((1, 1)))).unwrap().found(&["x".to_string()]));
    assert!(compile("(", true).unwrap_err().starts_with("invalid regex \"(\": regex parse error:"));
    assert!(compile("(", false).unwrap().is_match("a ( b"));
}

#[test]
fn text_and_gone_wait_on_the_screen_until_the_program_exits() {
    let mut scene = Scene::new("hello world");
    let both = |scene: &Scene, wanted: &str| {
        [scene.verdict(&Wait::Text(text(wanted, false, None))), scene.verdict(&Wait::Gone(text(wanted, false, None)))]
    };
    assert_eq!([both(&scene, "world"), both(&scene, "bye")], [["met", "pending"], ["pending", "met"]]);
    scene.exited = true;
    assert_eq!([both(&scene, "world"), both(&scene, "bye")], [["met", "never"], ["never", "met"]]);
}

#[test]
fn stable_waits_for_quiet_screen_changes_and_idle_for_quiet_output() {
    let mut scene = Scene::new("");
    scene.last_change = scene.at(10);
    scene.last_output = scene.at(90);
    let quiet = |scene: &Scene, ms| {
        [Wait::Stable { quiet_ms: ms }, Wait::Idle { quiet_ms: ms }].map(|wait| scene.verdict(&wait))
    };
    assert_eq!(
        [quiet(&scene, 0), quiet(&scene, 20), quiet(&scene, 1_000)],
        [["met", "met"], ["met", "pending until 110ms"], ["pending until 1010ms", "pending until 1090ms"]]
    );
    scene.exited = true;
    assert_eq!(quiet(&scene, 1_000), ["met", "met"]);
}

#[test]
fn quiet_is_counted_from_the_action_when_it_came_after_the_last_change() {
    let scene = Scene::new("");
    let stable = |action: Option<u32>| {
        let condition = Condition::new(&Wait::Stable { quiet_ms: 50 }).unwrap();
        let condition = match action {
            Some(ms) => condition.counted_from(scene.at(ms)),
            None => condition,
        };
        scene.judge(&condition, scene.at(100))
    };
    assert_eq!([stable(None), stable(Some(80)), stable(Some(0))], ["met", "pending until 130ms", "met"]);
    let idle = Condition::new(&Wait::Idle { quiet_ms: 50 }).unwrap().counted_from(scene.at(80));
    assert_eq!(scene.judge(&idle, scene.at(100)), "pending until 130ms");
}

#[test]
fn a_quiet_wait_is_judged_no_later_than_its_deadline() {
    let scene = Scene::new("");
    let stable = Condition::new(&Wait::Stable { quiet_ms: 50 }).unwrap();
    assert_eq!([scene.judge(&stable, scene.at(40)), scene.judge(&stable, scene.at(50))], ["pending until 50ms", "met"]);
}

#[test]
fn exit_cursor_and_frame_wait_for_their_state() {
    let mut scene = Scene::new("ab\r\ncd");
    let verdicts = |scene: &Scene| {
        [
            Wait::Exit { code: None },
            Wait::Cursor { row: 2, col: 3 },
            Wait::Cursor { row: 2, col: 2 },
            Wait::Cursor { row: 1, col: 3 },
            Wait::Frame { frame: 5 },
            Wait::Frame { frame: 6 },
        ]
        .map(|wait| scene.verdict(&wait))
    };
    assert_eq!(verdicts(&scene), ["pending", "met", "pending", "pending", "met", "pending"]);
    scene.exited = true;
    assert_eq!(verdicts(&scene), ["met", "met", "never", "never", "met", "never"]);
}

#[test]
fn elapsed_is_met_once_its_time_has_come() {
    let scene = Scene::new("");
    let elapsed = |until: Instant, deadline: Instant| scene.judge(&Condition::Elapsed(until), deadline);
    assert_eq!(
        [elapsed(scene.at(50), scene.at(60)), elapsed(scene.at(50), scene.at(50)), elapsed(scene.at(50), scene.at(40))],
        ["met", "met", "pending until 50ms"]
    );
    let later = Condition::new(&Wait::Elapsed { ms: 3_600_000 }).unwrap();
    assert_eq!(scene.judge(&later, scene.at(7_200_000)).split_whitespace().next(), Some("pending"));
    let now = Condition::new(&Wait::Elapsed { ms: 0 }).unwrap();
    assert_eq!(scene.judge(&now, scene.at(7_200_000)), "met");
}

#[test]
fn a_command_wait_is_met_at_a_ready_prompt_and_a_prompt_wait_at_a_later_one() {
    let mut scene = Scene::new("");
    let verdicts =
        |scene: &Scene| [scene.verdict(&Wait::Command), scene.judge(&Condition::PromptAfter(2), scene.at(100))];
    let mut seen = vec![verdicts(&scene)];
    for (ready, prompts) in [(false, 3), (true, 2), (true, 3)] {
        scene.prompt = Some(PromptState { ready, prompts });
        seen.push(verdicts(&scene));
    }
    assert_eq!(seen, [["pending", "pending"], ["pending", "pending"], ["met", "pending"], ["met", "met"]]);
}
