use std::path::Path;

use super::picture::{pixel, png_size};
use crate::support::{AgentTerminal, Run, fixture};

#[test]
fn diff_screenshot_passes_when_the_screen_still_looks_like_the_baseline() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui")]).success();
    let work = at.work_folder();
    at.run_in(&work, &["screenshot", "before.png"]).success();
    let frame = at.frame("work");

    let run = at.run_in(&work, &["diff", "screenshot", "--baseline", "before.png"]).success();

    let image = at.home().join("screenshots").join(format!("work-t1-{frame}-diff.png"));
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

    let changed = the_diff_above_the_threshold_fails_naming_its_image(&failed, &work);
    the_diff_image_marks_only_what_changed(&work);
    let json = within.success().json();
    assert_eq!(json["changed"], changed);
    assert_eq!(json["pixels"], 84600);
    assert_eq!(json["threshold"], 0.5);
    assert_eq!(json["passed"], true);
}

fn the_diff_above_the_threshold_fails_naming_its_image(failed: &Run, work: &Path) -> u64 {
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
    changed
}

fn the_diff_image_marks_only_what_changed(work: &Path) {
    let marked = work.join("changes.png");
    let beta_row = (10 + 10, 10 + 2 * 20 + 10);
    let untouched = (10 + 30 * 9 + 4, 10 + 6 * 20 + 10);
    assert_eq!(pixel(&marked, (beta_row.0, beta_row.1)), CHANGED);
    assert_ne!(pixel(&marked, (untouched.0, untouched.1)), CHANGED);
}
