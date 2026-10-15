use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Metric {
    RoundTrip,
    Snapshot,
    Png,
    PressWaitText,
    IdleMemory,
    TabMemory,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Milliseconds,
    Megabytes,
}

impl Unit {
    pub fn symbol(self) -> &'static str {
        match self {
            Unit::Milliseconds => "ms",
            Unit::Megabytes => "MB",
        }
    }

    pub fn tolerance(self) -> Tolerance {
        match self {
            Unit::Milliseconds => Tolerance { percent: 25.0, amount: 0.25 },
            Unit::Megabytes => Tolerance { percent: 10.0, amount: 0.25 },
        }
    }
}

#[derive(Clone, Copy)]
pub struct Tolerance {
    pub percent: f64,
    pub amount: f64,
}

impl Metric {
    pub const ALL: [Metric; 6] = [
        Metric::RoundTrip,
        Metric::Snapshot,
        Metric::Png,
        Metric::PressWaitText,
        Metric::IdleMemory,
        Metric::TabMemory,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Metric::RoundTrip => "CLI round trip (`get frame`)",
            Metric::Snapshot => "`snapshot`",
            Metric::Png => "`screenshot` PNG at 120x40",
            Metric::PressWaitText => "`press --wait <text>` after the text appears",
            Metric::IdleMemory => "idle daemon memory",
            Metric::TabMemory => "memory per active tab",
        }
    }

    pub fn unit(self) -> Unit {
        match self {
            Metric::RoundTrip | Metric::Snapshot | Metric::Png | Metric::PressWaitText => Unit::Milliseconds,
            Metric::IdleMemory | Metric::TabMemory => Unit::Megabytes,
        }
    }

    pub fn target(self) -> Option<Target> {
        match self {
            Metric::RoundTrip => Some(Target::Limit(5.0)),
            Metric::Snapshot => Some(Target::Limit(10.0)),
            Metric::Png => Some(Target::Limit(40.0)),
            Metric::PressWaitText => None,
            Metric::IdleMemory => Some(Target::Limit(10.0)),
            Metric::TabMemory => Some(Target::Reference(5.0)),
        }
    }
}

#[derive(Clone, Copy)]
pub enum Target {
    Limit(f64),
    Reference(f64),
}

impl Target {
    pub fn value(self) -> f64 {
        match self {
            Target::Limit(value) | Target::Reference(value) => value,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Results {
    pub metrics: Vec<Samples>,
}

#[derive(Serialize, Deserialize)]
pub struct Samples {
    pub metric: Metric,
    pub base: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_base: Option<String>,
    pub change: Vec<f64>,
}
