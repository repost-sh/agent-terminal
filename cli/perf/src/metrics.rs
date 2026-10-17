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

    pub(crate) fn tolerance(self) -> Tolerance {
        match self {
            Metric::RoundTrip | Metric::Snapshot => Tolerance { percent: 10.0, amount: 0.15 },
            Metric::Png => Tolerance { percent: 12.0, amount: 1.0 },
            Metric::PressWaitText => Tolerance { percent: 15.0, amount: 0.08 },
            Metric::IdleMemory => Tolerance { percent: 5.0, amount: 0.5 },
            Metric::TabMemory => Tolerance { percent: 3.0, amount: 0.15 },
        }
    }

    pub(crate) fn target(self) -> Option<Target> {
        match self {
            Metric::RoundTrip => Some(Target::Limit { limit: 5.0, margin: 0.1 }),
            Metric::Snapshot => Some(Target::Limit { limit: 10.0, margin: 0.1 }),
            Metric::Png => Some(Target::Limit { limit: 40.0, margin: 1.0 }),
            Metric::PressWaitText => None,
            Metric::IdleMemory => Some(Target::Limit { limit: 10.0, margin: 0.25 }),
            Metric::TabMemory => Some(Target::Reference(5.0)),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Target {
    Limit { limit: f64, margin: f64 },
    Reference(f64),
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
