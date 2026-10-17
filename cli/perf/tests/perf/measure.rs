use std::path::Path;
use std::process::Output;

use serde_json::Value;

use crate::programs::{at_perf, bins, stderr};

const RUNS: &str = "3";
const MEMORY_RUNS: &str = "1";

const METRICS: [(&str, usize); 6] =
    [("round-trip", 3), ("snapshot", 3), ("png", 3), ("press-wait-text", 3), ("idle-memory", 1), ("tab-memory", 1)];

struct Measured {
    output: Output,
    results: Option<Value>,
}

fn measure(base: &Path, change: &Path) -> Measured {
    let dir = tempfile::tempdir().expect("scratch folder");
    let out = dir.path().join("results.json");
    let output = at_perf([
        "measure".as_ref(),
        "--base".as_ref(),
        base.as_os_str(),
        "--change".as_ref(),
        change.as_os_str(),
        "--fixtures".as_ref(),
        bins().as_os_str(),
        "--work".as_ref(),
        dir.path().join("work").as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
        "--runs".as_ref(),
        RUNS.as_ref(),
        "--memory-runs".as_ref(),
        MEMORY_RUNS.as_ref(),
    ]);
    let results = std::fs::read_to_string(&out).ok().map(|json| serde_json::from_str(&json).expect("results are JSON"));
    Measured { output, results }
}

fn samples<'a>(metric: &'a Value, side: &str) -> &'a [Value] {
    metric[side].as_array().map_or(&[], Vec::as_slice)
}

fn shape(metric: &Value) -> (String, usize, Option<String>, usize) {
    (
        metric["metric"].as_str().unwrap_or_default().to_string(),
        samples(metric, "base").len(),
        metric.get("no_base").map(ToString::to_string),
        samples(metric, "change").len(),
    )
}

fn metrics(measured: Measured) -> Vec<Value> {
    let results = measured.results.expect("the results file");
    results["metrics"].as_array().expect("a list of metrics").clone()
}

fn outside(samples: impl Iterator<Item = Value>, wanted: impl Fn(f64) -> bool) -> Vec<Value> {
    samples.filter(|sample| !sample.as_f64().is_some_and(&wanted)).collect()
}

#[test]
fn measuring_a_binary_against_itself_samples_every_metric_on_both_sides() {
    let agent = bins().join("agent-terminal");
    let measured = measure(&agent, &agent);
    assert_eq!(measured.output.status.code(), Some(0), "{}", stderr(&measured.output));
    let metrics = metrics(measured);
    let expected: Vec<_> = METRICS.iter().map(|&(name, runs)| (name.to_string(), runs, None, runs)).collect();
    assert_eq!(metrics.iter().map(shape).collect::<Vec<_>>(), expected);
    let every = metrics.iter().flat_map(|metric| [samples(metric, "base"), samples(metric, "change")].concat());
    assert_eq!(outside(every, f64::is_finite), Vec::<Value>::new());
    let timings = metrics.iter().take(4).flat_map(|metric| samples(metric, "change").to_vec());
    assert_eq!(outside(timings, |ms| ms > 0.0), Vec::<Value>::new());
}

#[test]
fn a_base_that_cannot_start_is_reported_and_the_change_is_still_measured() {
    let measured = measure(&bins().join("line-printer"), &bins().join("agent-terminal"));
    assert_eq!(measured.output.status.code(), Some(0), "{}", stderr(&measured.output));
    let reason = Some(r#""the base daemon did not start: \"\"""#.to_string());
    let expected: Vec<_> = METRICS.iter().map(|&(name, runs)| (name.to_string(), 0, reason.clone(), runs)).collect();
    assert_eq!(metrics(measured).iter().map(shape).collect::<Vec<_>>(), expected);
}

#[test]
fn a_change_that_cannot_start_fails_the_measurement() {
    let agent = bins().join("agent-terminal");
    let measured = measure(&agent, &bins().join("line-printer"));
    assert_eq!(stderr(&measured.output), "error: the change daemon did not start: \"\"\n");
    assert_eq!(measured.output.status.code(), Some(1));
    assert!(measured.results.is_none());
}
