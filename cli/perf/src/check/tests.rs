use super::{Report, check};
use crate::metrics::{Metric, Results, Samples};

fn results(judged: Samples) -> Results {
    let steady = |metric| Samples { metric, base: vec![1.0], no_base: None, change: vec![1.0] };
    let mut metrics: Vec<Samples> =
        Metric::ALL.iter().filter(|&&metric| metric != judged.metric).map(|&metric| steady(metric)).collect();
    metrics.push(judged);
    Results { metrics }
}

fn judged(metric: Metric, base: &[f64], change: &[f64]) -> Report {
    check(&results(Samples { metric, base: base.to_vec(), no_base: None, change: change.to_vec() })).unwrap()
}

fn verdict(report: &Report, metric: Metric) -> String {
    let row = report.text.lines().find(|line| line.starts_with(&format!("| {} |", metric.label()))).unwrap();
    row.trim_end_matches(" |").rsplit("| ").next().unwrap().to_string()
}

fn rounds(base: f64, slower: f64) -> (Vec<f64>, Vec<f64>) {
    let base = vec![base - 0.01, base, base + 0.01, base, base];
    let change = base.iter().map(|ms| ms + slower).collect();
    (base, change)
}

#[test]
fn a_phase_flip_between_the_sides_passes_when_each_round_is_close_to_its_base() {
    let base = [2.0, 2.0, 2.0, 2.0, 2.0, 3.6, 3.6, 3.6, 3.6];
    let change = [2.05, 2.05, 2.05, 2.05, 3.65, 3.65, 3.65, 3.65, 3.65];
    let report = judged(Metric::RoundTrip, &base, &change);
    assert!(
        report.text.contains("| CLI round trip (`get frame`) | ≤ 5 ms (margin 0.1 ms) | 2.00 ms | 3.65 ms | +0.05 ms (+2.5%) | +10% and +0.15 ms | ok |\n"),
        "{}",
        report.text
    );
    assert!(report.passed, "{}", report.text);
}

const THRESHOLDS: [(Metric, f64, f64, f64); 12] = [
    (Metric::RoundTrip, 2.0, 0.19, 0.21),
    (Metric::RoundTrip, 1.0, 0.14, 0.16),
    (Metric::Snapshot, 2.0, 0.19, 0.21),
    (Metric::Snapshot, 1.0, 0.14, 0.16),
    (Metric::Png, 10.0, 1.15, 1.25),
    (Metric::Png, 5.0, 0.95, 1.05),
    (Metric::PressWaitText, 1.0, 0.14, 0.16),
    (Metric::PressWaitText, 0.4, 0.07, 0.09),
    (Metric::IdleMemory, 12.0, 0.55, 0.65),
    (Metric::IdleMemory, 8.0, 0.45, 0.55),
    (Metric::TabMemory, 6.0, 0.17, 0.19),
    (Metric::TabMemory, 4.0, 0.14, 0.16),
];

#[test]
fn each_metric_regresses_only_past_both_its_amount_and_its_percent() {
    for (metric, base, within, past) in THRESHOLDS {
        let (samples, slower) = rounds(base, within);
        let report = judged(metric, &samples, &slower);
        assert!(report.passed && !verdict(&report, metric).contains("regressed"), "{base} + {within}: {}", report.text);
        let (samples, slower) = rounds(base, past);
        let report = judged(metric, &samples, &slower);
        assert!(!report.passed && verdict(&report, metric).ends_with("regressed"), "{base} + {past}: {}", report.text);
    }
}

const LIMITS: [(Metric, f64, f64, bool, &str); 14] = [
    (Metric::RoundTrip, 4.95, 5.11, false, "over target"),
    (Metric::RoundTrip, 4.95, 5.09, true, "ok"),
    (Metric::Snapshot, 9.9, 10.15, false, "over target"),
    (Metric::Snapshot, 10.0, 10.2, false, "over target"),
    (Metric::Snapshot, 9.95, 10.05, true, "ok"),
    (Metric::Snapshot, 12.0, 12.0, true, "over target, base too"),
    (Metric::Snapshot, 12.0, 9.0, true, "ok"),
    (Metric::Png, 39.5, 41.1, false, "over target"),
    (Metric::Png, 39.5, 40.9, true, "ok"),
    (Metric::Png, 39.5, 41.0, true, "ok"),
    (Metric::IdleMemory, 9.9, 10.3, false, "over target"),
    (Metric::IdleMemory, 9.9, 10.2, true, "ok"),
    (Metric::IdleMemory, 9.9, 10.25, true, "ok"),
    (Metric::TabMemory, 4.9, 5.0, true, "ok"),
];

#[test]
fn a_limit_fails_only_the_change_that_crosses_it_past_its_margin() {
    for (metric, base, change, passed, expected) in LIMITS {
        let report = judged(metric, &[base], &[change]);
        assert_eq!((report.passed, verdict(&report, metric)), (passed, expected.to_string()), "{}", report.text);
    }
}

#[test]
fn a_limit_the_base_is_already_over_is_a_warning_line() {
    let report = judged(Metric::Snapshot, &[12.0, 12.5, 11.5], &[11.0, 12.6, 12.0]);
    assert!(
        report.text.ends_with(
            "\nOver its target on the base too, so a warning rather than a failure:\n\
             - `snapshot` is 12.00 ms, over its 10 ms target, and the base was already over it with 12.00 ms\n"
        ),
        "{}",
        report.text
    );
    assert!(report.passed);
}

#[test]
fn memory_per_tab_over_its_reference_is_shown_but_passes() {
    let report = judged(Metric::TabMemory, &[5.1], &[5.2]);
    assert_eq!((report.passed, verdict(&report, Metric::TabMemory)), (true, "over reference".to_string()));
}

#[test]
fn a_metric_without_a_base_is_held_to_its_limit_without_the_margin() {
    let held = |change: f64| {
        let png = Samples {
            metric: Metric::Png,
            base: vec![],
            no_base: Some("no `screenshot`".into()),
            change: vec![change],
        };
        check(&results(png)).unwrap()
    };
    let over = held(40.5);
    assert!(over.text.contains("\n- `screenshot` PNG at 120x40 is 40.50 ms, over its 40 ms target\n"), "{}", over.text);
    assert!(!over.passed);
    assert!(held(40.0).passed);
}

#[test]
fn only_the_rounds_both_sides_measured_are_paired_and_the_report_says_so() {
    let report = judged(Metric::RoundTrip, &[2.0, 2.0], &[2.0, 2.0, 9.0]);
    assert!(
        report.text.ends_with(
            "\nPaired only the rounds both sides measured:\n\
             - CLI round trip (`get frame`): 2 paired; the base has 2 samples and this change 3\n"
        ),
        "{}",
        report.text
    );
    assert!(report.passed);
}

const REPORT: &str = "| metric | target | base | this change | paired difference | tolerance | verdict |\n\
                     |---|---|---|---|---|---|---|\n\
                     | CLI round trip (`get frame`) | ≤ 5 ms (margin 0.1 ms) | 2.00 ms | 2.60 ms | +0.60 ms (+30.0%) | +10% and +0.15 ms | regressed |\n\
                     | `snapshot` | ≤ 10 ms (margin 0.1 ms) | 9.90 ms | 10.20 ms | +0.30 ms (+3.0%) | +10% and +0.15 ms | over target |\n\
                     | `screenshot` PNG at 120x40 | ≤ 40 ms (margin 1 ms) | 6.00 ms | 6.00 ms | +0.00 ms (+0.0%) | +12% and +1 ms | ok |\n\
                     | `press --wait <text>` after the text appears | none | no base | 0.40 ms | no base | +15% and +0.08 ms | ok |\n\
                     | idle daemon memory | ≤ 10 MB (margin 0.25 MB) | 11.00 MB | 11.10 MB | +0.10 MB (+0.9%) | +5% and +0.5 MB | over target, base too |\n\
                     | memory per active tab | about 5 MB (reference) | 4.00 MB | 4.10 MB | +0.10 MB (+2.5%) | +3% and +0.15 MB | ok |\n\
                     \n\
                     - CLI round trip (`get frame`) regressed: +0.60 ms per round against 2.00 ms on the base (+30.0%; the tolerance is +10% and +0.15 ms)\n\
                     - `snapshot` is 10.20 ms, over its 10 ms target by more than its 0.1 ms margin, while the base is 9.90 ms\n\
                     \n\
                     Over its target on the base too, so a warning rather than a failure:\n\
                     - idle daemon memory is 11.10 MB, over its 10 MB target, and the base was already over it with 11.00 MB\n\
                     \n\
                     Paired only the rounds both sides measured:\n\
                     - memory per active tab: 1 paired; the base has 2 samples and this change 1\n\
                     \n\
                     No base to compare with, so no regression check:\n\
                     - `press --wait <text>` after the text appears: no `press`\n";

#[test]
fn the_report_shows_each_paired_difference_and_the_tolerance_it_was_judged_by() {
    let report = check(&Results {
        metrics: vec![
            Samples {
                metric: Metric::RoundTrip,
                base: vec![2.0, 2.2, 1.8],
                no_base: None,
                change: vec![2.6, 2.6, 2.6],
            },
            Samples { metric: Metric::Snapshot, base: vec![9.9], no_base: None, change: vec![10.2] },
            Samples { metric: Metric::Png, base: vec![6.0], no_base: None, change: vec![6.0] },
            Samples {
                metric: Metric::PressWaitText,
                base: vec![],
                no_base: Some("no `press`".into()),
                change: vec![0.4],
            },
            Samples { metric: Metric::IdleMemory, base: vec![11.0], no_base: None, change: vec![11.1] },
            Samples { metric: Metric::TabMemory, base: vec![4.0, 4.0], no_base: None, change: vec![4.1] },
        ],
    })
    .unwrap();
    assert_eq!(report.text, REPORT);
    assert!(!report.passed);
}

#[test]
fn a_side_without_samples_is_an_error_that_names_it() {
    let refusal = |base: Vec<f64>, change: Vec<f64>| {
        let snapshot = Samples { metric: Metric::Snapshot, base, no_base: None, change };
        format!("{:#}", check(&results(snapshot)).err().unwrap())
    };
    assert_eq!(refusal(vec![], vec![2.0]), "`snapshot` has no samples of the base");
    assert_eq!(refusal(vec![2.0], vec![]), "`snapshot` has no samples of the change");
}
