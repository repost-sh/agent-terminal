use anyhow::{Context, Result};

use crate::metrics::{Metric, Results, Samples, Target};

pub(crate) struct Report {
    pub text: String,
    pub passed: bool,
}

struct Compared {
    median: f64,
    difference: f64,
    rounds: usize,
}

impl Compared {
    fn of(samples: &Samples) -> Option<Compared> {
        let differences: Vec<f64> =
            samples.base.iter().zip(&samples.change).map(|(base, change)| change - base).collect();
        Some(Compared { median: median(&samples.base)?, difference: median(&differences)?, rounds: samples.base.len() })
    }

    fn percent(&self) -> f64 {
        self.difference / self.median * 100.0
    }
}

enum Base {
    Measured(Compared),
    Missing(String),
}

enum Over {
    Limit { limit: f64, margin: f64 },
    LimitOnTheBase { limit: f64, base: f64 },
    Reference,
}

impl Over {
    fn label(&self) -> &'static str {
        match self {
            Over::Limit { .. } => "over target",
            Over::LimitOnTheBase { .. } => "over target, base too",
            Over::Reference => "over reference",
        }
    }
}

struct Row {
    metric: Metric,
    base: Base,
    change: f64,
    rounds: usize,
}

impl Row {
    fn line(&self) -> String {
        format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            self.metric.label(),
            self.target(),
            self.base(),
            self.amount(self.change),
            self.difference(),
            self.tolerance(),
            self.verdict()
        )
    }

    fn over(&self) -> Option<Over> {
        match self.metric.target()? {
            Target::Limit { limit, margin } => self.over_limit(limit, margin),
            Target::Reference(reference) => (self.change > reference).then_some(Over::Reference),
        }
    }

    fn over_limit(&self, limit: f64, margin: f64) -> Option<Over> {
        match &self.base {
            Base::Missing(_) => (self.change > limit).then_some(Over::Limit { limit, margin }),
            Base::Measured(base) if base.median > limit => {
                (self.change > limit).then_some(Over::LimitOnTheBase { limit, base: base.median })
            }
            Base::Measured(_) => (self.change > limit + margin).then_some(Over::Limit { limit, margin }),
        }
    }

    fn regressed(&self) -> Option<&Compared> {
        let Base::Measured(base) = &self.base else { return None };
        let tolerance = self.metric.tolerance();
        (base.difference > tolerance.amount && base.percent() > tolerance.percent).then_some(base)
    }

    fn verdict(&self) -> String {
        let over = self.over().map(|over| over.label());
        let verdict: Vec<&str> = over.into_iter().chain(self.regressed().map(|_| "regressed")).collect();
        if verdict.is_empty() { "ok".to_string() } else { verdict.join(", ") }
    }

    fn failures(&self) -> Vec<String> {
        let mut failures = Vec::new();
        if let Some(Over::Limit { limit, margin }) = self.over() {
            failures.push(self.crossed(limit, margin));
        }
        if let Some(base) = self.regressed() {
            failures.push(format!(
                "{} regressed: {} per round against {} on the base ({:+.1}%; the tolerance is {})",
                self.metric.label(),
                self.signed(base.difference),
                self.amount(base.median),
                base.percent(),
                self.tolerance()
            ));
        }
        failures
    }

    fn crossed(&self, limit: f64, margin: f64) -> String {
        let unit = self.metric.unit().symbol();
        let over = format!("{} is {}, over its {limit} {unit} target", self.metric.label(), self.amount(self.change));
        match &self.base {
            Base::Measured(base) => {
                format!(
                    "{over} by more than its {margin} {unit} margin, while the base is {}",
                    self.amount(base.median)
                )
            }
            Base::Missing(_) => over,
        }
    }

    fn warning(&self) -> Option<String> {
        let Some(Over::LimitOnTheBase { limit, base }) = self.over() else { return None };
        Some(format!(
            "- {} is {}, over its {limit} {} target, and the base was already over it with {}\n",
            self.metric.label(),
            self.amount(self.change),
            self.metric.unit().symbol(),
            self.amount(base)
        ))
    }

    fn unpaired(&self) -> Option<String> {
        let Base::Measured(base) = &self.base else { return None };
        (base.rounds != self.rounds).then(|| {
            format!(
                "- {}: {} paired; the base has {} samples and this change {}\n",
                self.metric.label(),
                base.rounds.min(self.rounds),
                base.rounds,
                self.rounds
            )
        })
    }

    fn missing(&self) -> Option<String> {
        match &self.base {
            Base::Missing(reason) => Some(format!("- {}: {reason}\n", self.metric.label())),
            Base::Measured(_) => None,
        }
    }

    fn amount(&self, value: f64) -> String {
        format!("{value:.2} {}", self.metric.unit().symbol())
    }

    fn signed(&self, value: f64) -> String {
        format!("{value:+.2} {}", self.metric.unit().symbol())
    }

    fn base(&self) -> String {
        match &self.base {
            Base::Measured(base) => self.amount(base.median),
            Base::Missing(_) => "no base".to_string(),
        }
    }

    fn difference(&self) -> String {
        match &self.base {
            Base::Measured(base) => format!("{} ({:+.1}%)", self.signed(base.difference), base.percent()),
            Base::Missing(_) => "no base".to_string(),
        }
    }

    fn tolerance(&self) -> String {
        let tolerance = self.metric.tolerance();
        format!("+{}% and +{} {}", tolerance.percent, tolerance.amount, self.metric.unit().symbol())
    }

    fn target(&self) -> String {
        let unit = self.metric.unit().symbol();
        match self.metric.target() {
            Some(Target::Limit { limit, margin }) => format!("≤ {limit} {unit} (margin {margin} {unit})"),
            Some(Target::Reference(reference)) => format!("about {reference} {unit} (reference)"),
            None => "none".to_string(),
        }
    }
}

pub(crate) fn check(results: &Results) -> Result<Report> {
    let rows = Metric::ALL.iter().map(|&metric| row(results, metric)).collect::<Result<Vec<_>>>()?;
    let mut text = String::from(
        "| metric | target | base | this change | paired difference | tolerance | verdict |\n|---|---|---|---|---|---|---|\n",
    );
    text.push_str(&rows.iter().map(Row::line).collect::<String>());
    let failures: Vec<String> = rows.iter().flat_map(Row::failures).collect();
    if !failures.is_empty() {
        text.push('\n');
        text.extend(failures.iter().flat_map(|failure| ["- ", failure.as_str(), "\n"]));
    }
    text.push_str(&section(
        "Over its target on the base too, so a warning rather than a failure:",
        rows.iter().filter_map(Row::warning),
    ));
    text.push_str(&section("Paired only the rounds both sides measured:", rows.iter().filter_map(Row::unpaired)));
    text.push_str(&section("No base to compare with, so no regression check:", rows.iter().filter_map(Row::missing)));
    Ok(Report { text, passed: failures.is_empty() })
}

fn section(heading: &str, lines: impl Iterator<Item = String>) -> String {
    let lines: String = lines.collect();
    if lines.is_empty() {
        return String::new();
    }
    format!("\n{heading}\n{lines}")
}

fn row(results: &Results, metric: Metric) -> Result<Row> {
    let samples = results
        .metrics
        .iter()
        .find(|samples| samples.metric == metric)
        .with_context(|| format!("the results have no samples of {}", metric.label()))?;
    let change = median(&samples.change).with_context(|| format!("{} has no samples of the change", metric.label()))?;
    let base = match &samples.no_base {
        Some(reason) => Base::Missing(reason.clone()),
        None => Base::Measured(
            Compared::of(samples).with_context(|| format!("{} has no samples of the base", metric.label()))?,
        ),
    };
    Ok(Row { metric, base, change, rounds: samples.change.len() })
}

fn median(samples: &[f64]) -> Option<f64> {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    let upper = *sorted.get(middle)?;
    if sorted.len() % 2 == 1 {
        return Some(upper);
    }
    let lower = *sorted.get(middle.checked_sub(1)?)?;
    Some(lower.midpoint(upper))
}

#[cfg(test)]
mod tests;
