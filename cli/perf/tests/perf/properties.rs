use proptest::collection::vec;
use proptest::prelude::Strategy;
use proptest::test_runner::Config;
use serde_json::json;

use crate::programs::at_perf;

const METRICS: [(&str, f64); 6] = [
    ("round-trip", 5.0),
    ("snapshot", 10.0),
    ("png", 40.0),
    ("press-wait-text", 1000.0),
    ("idle-memory", 10.0),
    ("tab-memory", 5.0),
];

fn samples() -> impl Strategy<Value = Vec<Vec<f64>>> {
    METRICS.map(|(_, limit)| vec(0.01..=limit, 1..8)).to_vec()
}

fn verdicts(samples: &[Vec<f64>], change: impl Fn(usize, &[f64]) -> Vec<f64>) -> (Option<i32>, Vec<String>) {
    let metrics: Vec<_> = METRICS
        .iter()
        .zip(samples)
        .enumerate()
        .map(|(at, ((metric, _), base))| json!({ "metric": metric, "base": base, "change": change(at, base) }))
        .collect();
    let dir = tempfile::tempdir().expect("scratch folder");
    let path = dir.path().join("results.json");
    std::fs::write(&path, json!({ "metrics": metrics }).to_string()).expect("write the results");
    let checked = at_perf(["check".as_ref(), path.as_os_str()]);
    let stdout = String::from_utf8_lossy(&checked.stdout).into_owned();
    let rows = stdout.lines().skip(2).take(6).map(|row| row.rsplit('|').nth(1).unwrap_or_default().trim().to_string());
    (checked.status.code(), rows.collect())
}

proptest::proptest! {
    #![proptest_config(Config { cases: 48, failure_persistence: None, ..Config::default() })]

    #[test]
    fn a_change_that_measures_the_same_as_its_base_within_the_targets_passes(samples in samples()) {
        let (code, rows) = verdicts(&samples, |_, base| base.to_vec());
        proptest::prop_assert_eq!(code, Some(0));
        proptest::prop_assert_eq!(rows, vec!["ok"; 6]);
    }

    #[test]
    fn a_round_trip_more_than_a_tenth_and_0_15_ms_slower_in_every_round_regresses(samples in samples()) {
        let slower = |at: usize, base: &[f64]| {
            if at == 0 { base.iter().map(|ms| ms * 1.1 + 0.16).collect() } else { base.to_vec() }
        };
        let (code, rows) = verdicts(&samples, slower);
        proptest::prop_assert_eq!(code, Some(2));
        proptest::prop_assert!(rows.first().is_some_and(|verdict| verdict.ends_with("regressed")), "{rows:?}");
        proptest::prop_assert_eq!(rows.get(1..), Some(&vec!["ok".to_string(); 5][..]));
    }
}
