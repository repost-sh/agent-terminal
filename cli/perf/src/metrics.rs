use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Metric {
    RoundTrip,
    Snapshot,
    Png,
    WaitText,
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
    pub const ALL: [Metric; 6] =
        [Metric::RoundTrip, Metric::Snapshot, Metric::Png, Metric::WaitText, Metric::IdleMemory, Metric::TabMemory];

    pub fn label(self) -> &'static str {
        match self {
            Metric::RoundTrip => "CLI round trip (`get frame`)",
            Metric::Snapshot => "`snapshot`",
            Metric::Png => "`screenshot` PNG at 120x40",
            Metric::WaitText => "`wait <text>` after the text appears",
            Metric::IdleMemory => "idle daemon memory",
            Metric::TabMemory => "memory per active tab",
        }
    }

    pub fn unit(self) -> Unit {
        match self {
            Metric::RoundTrip | Metric::Snapshot | Metric::Png | Metric::WaitText => Unit::Milliseconds,
            Metric::IdleMemory | Metric::TabMemory => Unit::Megabytes,
        }
    }

    pub fn target(self) -> Option<f64> {
        match self {
            Metric::RoundTrip => Some(5.0),
            Metric::Snapshot => Some(10.0),
            Metric::Png => Some(40.0),
            Metric::WaitText => None,
            Metric::IdleMemory => Some(10.0),
            Metric::TabMemory => Some(5.0),
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
    pub change: Vec<f64>,
}
