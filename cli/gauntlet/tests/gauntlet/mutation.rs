use serde_json::{Value, json};

use crate::scratch::run;

const FIX: &str = "write a test whose expected value changes when the mutated line does, through the crate's real surface → GAUNTLET.md#mutation";

fn mutant(name: &str, summary: &str) -> Value {
    json!({ "scenario": { "Mutant": { "name": name } }, "summary": summary })
}

fn report(baseline: &str, mutants: &[Value], total: usize) -> Value {
    let mut outcomes = vec![json!({ "scenario": "Baseline", "summary": baseline })];
    outcomes.extend_from_slice(mutants);
    json!({ "total_mutants": total, "outcomes": outcomes })
}

fn checked(reports: &[Value]) -> (i32, String) {
    let dir = tempfile::tempdir().expect("scratch folder");
    let paths: Vec<String> = reports
        .iter()
        .enumerate()
        .map(|(at, report)| {
            let path = dir.path().join(format!("outcomes-{at}.json"));
            std::fs::write(&path, report.to_string()).expect("write the outcomes");
            path.display().to_string()
        })
        .collect();
    let mut args = vec!["mutation"];
    args.extend(paths.iter().map(String::as_str));
    let ran = run(dir.path(), &args);
    (ran.code, ran.stdout.replace(&format!("{}/", dir.path().display()), ""))
}

#[test]
fn shards_that_catch_every_viable_mutant_pass_together() {
    let first = report(
        "Success",
        &[mutant("a.rs:1:1: replace f with ()", "CaughtMutant"), mutant("a.rs:2:1: x", "Unviable")],
        2,
    );
    let second = report("Success", &[mutant("b.rs:1:1: replace g with ()", "CaughtMutant")], 1);
    assert_eq!(
        checked(&[first, second]),
        (0, "3 mutants: 2 caught, 1 unviable, 0 timed out, none missed\n".to_string())
    );
}

#[test]
fn a_missed_mutant_is_named() {
    let read = report(
        "Success",
        &[mutant("a.rs:1:1: replace f with ()", "CaughtMutant"), mutant("a.rs:3:9: replace > with >=", "MissedMutant")],
        2,
    );
    assert_eq!(
        checked(&[read]),
        (2, format!("a.rs:3:9: replace > with >= survived (MissedMutant): {FIX}\n1 offense(s)\n"))
    );
}

#[test]
fn a_failed_baseline_a_short_run_or_nothing_caught_proves_nothing() {
    let read = report("Failure", &[mutant("a.rs:1:1: f", "Unviable")], 3);
    assert_eq!(
        checked(&[read]),
        (
            2,
            format!(
                "outcomes-0.json: the unmutated baseline did not pass, so no mutant there proves anything: {FIX}\n\
                 outcomes-0.json: 1 of its 3 mutants were tested: {FIX}\n\
                 no mutant was caught, so the tests ran on nothing: {FIX}\n\
                 3 offense(s)\n"
            )
        )
    );
}

#[test]
fn more_than_five_percent_timeouts_fail_the_run() {
    let mut mutants: Vec<Value> = (0..19).map(|at| mutant(&format!("a.rs:{at}:1: f"), "CaughtMutant")).collect();
    mutants.push(mutant("a.rs:99:1: loop", "Timeout"));
    assert_eq!(checked(&[report("Success", &mutants, 20)]).0, 0);
    mutants.push(mutant("a.rs:98:1: loop", "Timeout"));
    assert_eq!(
        checked(&[report("Success", &mutants, 21)]),
        (
            2,
            format!(
                "2 of 21 mutants timed out, over 5%: a timeout hides whether the tests catch it; run with less load: {FIX}\n1 offense(s)\n"
            )
        )
    );
}

#[test]
fn a_report_that_cannot_be_read_is_an_error() {
    let dir = tempfile::tempdir().expect("scratch folder");
    std::fs::write(dir.path().join("broken.json"), "{").expect("write a broken report");
    let ran = run(dir.path(), &["mutation", "broken.json"]);
    assert!(ran.stderr.starts_with("error: parse broken.json: "), "{}", ran.stderr);
    assert_eq!(ran.code, 1);
}
