use std::process::Output;

use serde_json::Value;

use crate::programs::{at_perf, bins, stderr};

struct Loaded {
    output: Output,
    rows: Option<Value>,
}

fn load(agent: &str, plan: &[&str]) -> Loaded {
    let dir = tempfile::tempdir().expect("scratch folder");
    let out = dir.path().join("rows.json");
    let mut args = vec![
        "load".into(),
        "--agent".into(),
        bins().join(agent).into_os_string(),
        "--fixtures".into(),
        bins().as_os_str().to_owned(),
        "--work".into(),
        dir.path().join("work").into_os_string(),
        "--out".into(),
        out.clone().into_os_string(),
    ];
    args.extend(plan.iter().map(Into::into));
    let output = at_perf(args);
    let rows = std::fs::read_to_string(&out).ok().map(|json| serde_json::from_str(&json).expect("rows are JSON"));
    Loaded { output, rows }
}

fn case(row: &Value) -> (String, String, u64, u64, bool) {
    let positive = |field: &str| row[field].as_f64().is_some_and(|value| value > 0.0);
    let measured = positive("seconds")
        && positive("daemon_frames_per_sec")
        && positive("program_frames_per_sec")
        && positive("daemon_rss_mb")
        && row["daemon_fds"].as_u64().is_some_and(|fds| fds > 0)
        && positive("stream_messages_per_sec") == (row["viewer"] == "dashboard");
    (
        row["repaint"].to_string(),
        row["viewer"].to_string(),
        row["hz"].as_u64().unwrap_or_default(),
        row["tabs"].as_u64().unwrap_or_default(),
        measured,
    )
}

fn leading_cells(line: &str) -> String {
    line.split('|').skip(1).take(4).map(str::trim).collect::<Vec<_>>().join(" | ")
}

#[test]
fn a_load_run_measures_one_row_per_case_and_prints_them_as_a_table() {
    let plan =
        ["--seconds", "1", "--rates", "30", "--tabs", "1", "--repaints", "partial,full", "--viewers", "none,dashboard"];
    let loaded = load("agent-terminal", &plan);
    assert_eq!(loaded.output.status.code(), Some(0), "{}", stderr(&loaded.output));
    let rows = loaded.rows.expect("the rows file");
    let cases: Vec<_> = rows.as_array().expect("a list of rows").iter().map(case).collect();
    let expected: Vec<_> = [("partial", "none"), ("partial", "dashboard"), ("full", "none"), ("full", "dashboard")]
        .map(|(repaint, viewer)| (format!("{repaint:?}"), format!("{viewer:?}"), 30, 1, true))
        .to_vec();
    assert_eq!(cases, expected, "{rows}");
    let stdout = String::from_utf8_lossy(&loaded.output.stdout).into_owned();
    assert_eq!(
        stdout.lines().map(leading_cells).collect::<Vec<_>>(),
        [
            "rate | repaint | tabs | viewer",
            "--- | --- | --- | ---",
            "30 Hz | partial | 1 | none",
            "30 Hz | partial | 1 | dashboard",
            "30 Hz | full | 1 | none",
            "30 Hz | full | 1 | dashboard"
        ]
    );
    assert!(
        stderr(&loaded.output).starts_with(stdout.lines().next().unwrap_or_default()),
        "{}",
        stderr(&loaded.output)
    );
}

#[test]
fn a_program_that_is_not_agent_terminal_fails_the_case_it_was_measuring() {
    let loaded = load(
        "line-printer",
        &["--seconds", "1", "--rates", "30", "--tabs", "1", "--repaints", "full", "--viewers", "none"],
    );
    assert!(
        stderr(&loaded.output)
            .ends_with("error: 30 Hz full x1 viewer none: the measured binary daemon did not start: \"\"\n"),
        "{}",
        stderr(&loaded.output)
    );
    assert_eq!(loaded.output.status.code(), Some(1));
    assert!(loaded.rows.is_none());
}
