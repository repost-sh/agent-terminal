use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Metric {
    RoundTrip,
    Snapshot,
    Png,
    PressWaitText,
    IdleMemory,
    TabMemory,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unit {
    Milliseconds,
    Megabytes,
}

impl Unit {
    pub(crate) fn symbol(self) -> &'static str {
        match self {
            Unit::Milliseconds => "ms",
            Unit::Megabytes => "MB",
        }
    }

    pub(crate) fn tolerance(self) -> Tolerance {
        match self {
            Unit::Milliseconds => Tolerance { percent: 25.0, amount: 0.25 },
            Unit::Megabytes => Tolerance { percent: 10.0, amount: 0.25 },
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Tolerance {
    pub percent: f64,
    pub amount: f64,
}

impl Metric {
    pub(crate) const ALL: [Metric; 6] = [
        Metric::RoundTrip,
        Metric::Snapshot,
        Metric::Png,
        Metric::PressWaitText,
        Metric::IdleMemory,
        Metric::TabMemory,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Metric::RoundTrip => "CLI round trip (`get frame`)",
            Metric::Snapshot => "`snapshot`",
            Metric::Png => "`screenshot` PNG at 120x40",
            Metric::PressWaitText => "`press --wait <text>` after the text appears",
            Metric::IdleMemory => "idle daemon memory",
            Metric::TabMemory => "memory per active tab",
        }
    }

    pub(crate) fn unit(self) -> Unit {
        match self {
            Metric::RoundTrip | Metric::Snapshot | Metric::Png | Metric::PressWaitText => Unit::Milliseconds,
            Metric::IdleMemory | Metric::TabMemory => Unit::Megabytes,
        }
    }

    pub(crate) fn target(self) -> Option<Target> {
        match self {
            Metric::RoundTrip => Some(Target::Limit(5.0)),
            Metric::Snapshot | Metric::IdleMemory => Some(Target::Limit(10.0)),
            Metric::Png => Some(Target::Limit(40.0)),
            Metric::PressWaitText => None,
            Metric::TabMemory => Some(Target::Reference(5.0)),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Target {
    Limit(f64),
    Reference(f64),
}

impl Target {
    pub(crate) fn value(self) -> f64 {
        match self {
            Target::Limit(value) | Target::Reference(value) => value,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Results {
    pub metrics: Vec<Samples>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Samples {
    pub metric: Metric,
    pub base: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_base: Option<String>,
    pub change: Vec<f64>,
}
