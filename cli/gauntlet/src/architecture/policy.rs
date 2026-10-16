use std::fmt;

use crate::workspace::Kind;

pub(super) const PRODUCT: &str = "agent-terminal";

pub(super) const ROOT: &str = "main";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Layer {
    Entry,
    Client,
    Daemon,
    Presentation,
    Contract,
}

pub(super) const MODULES: &[(&str, Layer)] = &[
    ("main", Layer::Entry),
    ("attach", Layer::Client),
    ("batch", Layer::Client),
    ("commands", Layer::Client),
    ("connection", Layer::Client),
    ("doctor", Layer::Client),
    ("mcp", Layer::Client),
    ("skills", Layer::Client),
    ("upgrade", Layer::Client),
    ("native", Layer::Daemon),
    ("output", Layer::Presentation),
    ("exit", Layer::Contract),
    ("failure", Layer::Contract),
    ("global_options", Layer::Contract),
    ("home", Layer::Contract),
    ("key", Layer::Contract),
    ("paint", Layer::Contract),
    ("protocol", Layer::Contract),
    ("session_id", Layer::Contract),
    ("version", Layer::Contract),
];

impl Layer {
    pub(super) fn of(module: &str) -> Option<Self> {
        MODULES.iter().find(|(name, _)| *name == module).map(|(_, layer)| *layer)
    }

    pub(super) fn may_use(self, other: Self) -> bool {
        match self {
            Layer::Entry => matches!(other, Layer::Client | Layer::Contract),
            Layer::Client => other != Layer::Entry,
            Layer::Daemon => matches!(other, Layer::Daemon | Layer::Presentation | Layer::Contract),
            Layer::Presentation => matches!(other, Layer::Presentation | Layer::Contract),
            Layer::Contract => other == Layer::Contract,
        }
    }
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Layer::Entry => "entry",
            Layer::Client => "client",
            Layer::Daemon => "daemon",
            Layer::Presentation => "presentation",
            Layer::Contract => "contract",
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    Product,
    BlackBoxTests,
    Tooling,
}

pub(super) const CRATES: &[(&str, Role)] = &[
    (PRODUCT, Role::Product),
    ("at-acceptance", Role::BlackBoxTests),
    ("at-fixtures", Role::BlackBoxTests),
    ("at-perf", Role::BlackBoxTests),
    ("at-gauntlet", Role::Tooling),
];

impl Role {
    pub(super) fn of(member: &str) -> Option<Self> {
        CRATES.iter().find(|(name, _)| *name == member).map(|(_, role)| *role)
    }

    pub(super) fn may_depend(self, kind: Kind, on: Self) -> bool {
        self == Role::Product && kind == Kind::Dev && on == Role::BlackBoxTests
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Role::Product => "product",
            Role::BlackBoxTests => "black-box tests",
            Role::Tooling => "tooling",
        })
    }
}
