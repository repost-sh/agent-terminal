use crate::support::{AgentTerminal, Cue, fixture};

fn open_with_cue(at: &AgentTerminal, cue: &Cue) {
    at.run(&[
        "open",
        "--size",
        "30x5",
        "--wait",
        "line 2",
        &fixture("line-printer"),
        "--lines",
        "2",
        "--cue",
        &cue.path(),
    ])
    .success();
    cue.print("");
}

fn frame(at: &AgentTerminal) -> u64 {
    at.run(&["--json", "get", "frame"]).success().json()["frame"].as_u64().unwrap()
}

#[test]
fn diff_snapshot_shows_the_rows_that_changed_since_the_last_snapshot() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    open_with_cue(&at, &cue);
    let before = at.run(&["--json", "snapshot"]).success().json()["frame"].as_u64().unwrap();
    cue.print("line 3\x1b[1;1Hfirst\n");
    let after = frame(&at);
    assert_eq!(
        at.run(&["diff", "snapshot"]).success().stdout(),
        format!("frame {before} → frame {after} · 2 rows changed\n-1│line 1\n+1│first1\n-3│\n+3│line 3\n")
    );
    assert_eq!(
        at.run(&["diff", "snapshot"]).success().stdout(),
        format!("frame {after} → frame {after} · no rows changed\n")
    );
}

#[test]
fn diff_snapshot_counts_open_as_a_snapshot_and_has_json() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    let opened = at
        .run(&[
            "--json",
            "open",
            "--size",
            "30x5",
            "--wait",
            "line 1",
            &fixture("line-printer"),
            "--lines",
            "1",
            "--cue",
            &cue.path(),
        ])
        .success()
        .json();
    cue.print("line 2\n");
    let after = frame(&at);
    let diff = at.run(&["--json", "diff", "snapshot"]).success().json();
    assert_eq!(
        diff,
        serde_json::json!({
            "since": { "frame": opened["frame"] },
            "frame": after,
            "changes": [{ "row": 2, "before": "", "after": "line 2" }],
        })
    );
}

#[test]
fn diff_snapshot_against_a_saved_baseline() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    open_with_cue(&at, &cue);
    let text = at.home().join("before.txt");
    let json = at.home().join("before.json");
    std::fs::write(&text, at.run(&["snapshot"]).success().stdout()).unwrap();
    std::fs::write(&json, at.run(&["--json", "snapshot"]).success().stdout()).unwrap();
    cue.print("line 3\n");
    let after = frame(&at);
    at.run(&["snapshot"]).success();

    let text = text.to_string_lossy().into_owned();
    assert_eq!(
        at.run(&["diff", "snapshot", "--baseline", &text]).success().stdout(),
        format!("{text} → frame {after} · 1 row changed\n-3│\n+3│line 3\n")
    );
    let json = json.to_string_lossy().into_owned();
    assert_eq!(
        at.run(&["diff", "snapshot", "--baseline", &json]).success().stdout(),
        format!("{json} → frame {after} · 1 row changed\n-3│\n+3│line 3\n")
    );
}

#[test]
fn diff_snapshot_refuses_a_baseline_that_is_not_a_snapshot() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    open_with_cue(&at, &cue);
    let missing = at.home().join("missing.txt").to_string_lossy().into_owned();
    let run = at.run(&["diff", "snapshot", "--baseline", &missing]);
    assert_eq!(run.code(), 1);
    assert!(run.stderr().starts_with(&format!("error: read the baseline {missing}: ")), "{}", run.stderr());

    let notes = at.home().join("notes.txt");
    std::fs::write(&notes, "just some notes\n").unwrap();
    let notes = notes.to_string_lossy().into_owned();
    let run = at.run(&["diff", "snapshot", "--baseline", &notes]);
    assert_eq!(run.code(), 1);
    assert_eq!(
        run.stderr(),
        format!("error: {notes} is not a snapshot; save one with `agent-terminal snapshot > {notes}`\n")
    );
}
