use anyhow::{Context, Result};

use crate::metrics::{Metric, Results, Target};

pub struct Report {
    pub text: String,
    pub passed: bool,
}

enum Base {
    Measured(f64),
    Missing(String),
}

struct Row {
    metric: Metric,
    base: Base,
    change: f64,
}

impl Row {
    fn over_target(&self) -> Option<Target> {
        self.metric.target().filter(|target| self.change > target.value())
    }

    fn increase(&self, base: f64) -> f64 {
        (self.change - base) / base * 100.0
    }

    fn regressed_from(&self) -> Option<f64> {
        let Base::Measured(base) = self.base else { return None };
        let tolerance = self.metric.unit().tolerance();
        (self.change - base > tolerance.amount && self.increase(base) > tolerance.percent).then_some(base)
    }

    fn verdict(&self) -> &'static str {
        match (self.over_target(), self.regressed_from().is_some()) {
            (None, false) => "ok",
            (Some(Target::Limit(_)), false) => "over target",
            (Some(Target::Reference(_)), false) => "over reference",
            (None, true) => "regressed",
            (Some(Target::Limit(_)), true) => "over target, regressed",
            (Some(Target::Reference(_)), true) => "over reference, regressed",
        }
    }

    fn failures(&self) -> Vec<String> {
        let unit = self.metric.unit();
        let mut failures = Vec::new();
        if let Some(Target::Limit(limit)) = self.over_target() {
            failures.push(format!(
                "{} is {}, over its {limit} {} target",
                self.metric.label(),
                self.amount(self.change),
                unit.symbol()
            ));
        }
        if let Some(base) = self.regressed_from() {
            let tolerance = unit.tolerance();
            failures.push(format!(
                "{} regressed: {} against {} on the base (+{:.0}%; the limit is +{}% and +{} {})",
                self.metric.label(),
                self.amount(self.change),
                self.amount(base),
                self.increase(base),
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

    fn base(&self) -> String {
        match self.base {
            Base::Measured(base) => self.amount(base),
            Base::Missing(_) => "no base".to_string(),
        }
    }

    fn target(&self) -> String {
        let unit = self.metric.unit().symbol();
        match self.metric.target() {
            Some(Target::Limit(limit)) => format!("≤ {limit} {unit}"),
            Some(Target::Reference(reference)) => format!("about {reference} {unit} (reference)"),
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
            row.base(),
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
    let missing: Vec<String> = rows
        .iter()
        .filter_map(|row| match &row.base {
            Base::Missing(reason) => Some(format!("- {}: {reason}\n", row.metric.label())),
            Base::Measured(_) => None,
        })
        .collect();
    if !missing.is_empty() {
        text.push_str("\nNo base to compare with, so no regression check:\n");
        text.push_str(&missing.concat());
    }
    Ok(Report { text, passed: failures.is_empty() })
}

fn row(results: &Results, metric: Metric) -> Result<Row> {
    let samples = results
        .metrics
        .iter()
        .find(|samples| samples.metric == metric)
        .with_context(|| format!("the results have no samples of {}", metric.label()))?;
    let base = match &samples.no_base {
        Some(reason) => Base::Missing(reason.clone()),
        None => Base::Measured(
            median(&samples.base).with_context(|| format!("{} has no samples of the base", metric.label()))?,
        ),
    };
    Ok(Row {
        metric,
        base,
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
