use anyhow::{Context, Result};

use crate::metrics::{Metric, Results};

pub struct Report {
    pub text: String,
    pub passed: bool,
}

struct Row {
    metric: Metric,
    base: f64,
    change: f64,
}

impl Row {
    fn over_target(&self) -> bool {
        self.metric.target().is_some_and(|target| self.change > target)
    }

    fn increase(&self) -> f64 {
        (self.change - self.base) / self.base * 100.0
    }

    fn regressed(&self) -> bool {
        let tolerance = self.metric.unit().tolerance();
        self.change - self.base > tolerance.amount && self.increase() > tolerance.percent
    }

    fn verdict(&self) -> &'static str {
        match (self.over_target(), self.regressed()) {
            (false, false) => "ok",
            (true, false) => "over target",
            (false, true) => "regressed",
            (true, true) => "over target, regressed",
        }
    }

    fn failures(&self) -> Vec<String> {
        let unit = self.metric.unit();
        let mut failures = Vec::new();
        if let Some(target) = self.metric.target().filter(|_| self.over_target()) {
            failures.push(format!(
                "{} is {}, over its {target} {} target",
                self.metric.label(),
                self.amount(self.change),
                unit.symbol()
            ));
        }
        if self.regressed() {
            let tolerance = unit.tolerance();
            failures.push(format!(
                "{} regressed: {} against {} on the base (+{:.0}%; the limit is +{}% and +{} {})",
                self.metric.label(),
                self.amount(self.change),
                self.amount(self.base),
                self.increase(),
                tolerance.percent,
                tolerance.amount,
                unit.symbol()
            ));
        }
        failures
    }

    fn amount(&self, value: f64) -> String {
        format!("{value:.2} {}", self.metric.unit().symbol())
    }

    fn target(&self) -> String {
        match self.metric.target() {
            Some(target) => format!("≤ {target} {}", self.metric.unit().symbol()),
            None => "none".to_string(),
        }
    }
}

pub fn check(results: &Results) -> Result<Report> {
    let rows = Metric::ALL.iter().map(|&metric| row(results, metric)).collect::<Result<Vec<_>>>()?;
    let mut text = String::from("| metric | target | base | this change | verdict |\n|---|---|---|---|---|\n");
    for row in &rows {
        text.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            row.metric.label(),
            row.target(),
            row.amount(row.base),
            row.amount(row.change),
            row.verdict()
        ));
    }
    let failures: Vec<String> = rows.iter().flat_map(Row::failures).collect();
    if !failures.is_empty() {
        text.push('\n');
    }
    for failure in &failures {
        text.push_str(&format!("- {failure}\n"));
    }
    Ok(Report { text, passed: failures.is_empty() })
}

fn row(results: &Results, metric: Metric) -> Result<Row> {
    let samples = results
        .metrics
        .iter()
        .find(|samples| samples.metric == metric)
        .with_context(|| format!("the results have no samples of {}", metric.label()))?;
    Ok(Row {
        metric,
        base: median(&samples.base).with_context(|| format!("{} has no samples of the base", metric.label()))?,
        change: median(&samples.change).with_context(|| format!("{} has no samples of the change", metric.label()))?,
    })
}

fn median(samples: &[f64]) -> Option<f64> {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    match sorted.len() {
        0 => None,
        n if n % 2 == 1 => Some(sorted[middle]),
        _ => Some((sorted[middle - 1] + sorted[middle]) / 2.0),
    }
}
