mod programs;
mod steps;
mod terminal;

use std::path::Path;

use gherkin_cargo_test::{Features, StepRegistry};
use libtest_mimic::Trial;

use crate::steps::{Define, register};

const FEATURES: [(&str, Define); 3] =
    [("input", steps::input::define), ("screen", steps::screen::define), ("sessions", steps::sessions::define)];

#[must_use]
pub fn trials(agent_terminal: &Path) -> Vec<Trial> {
    programs::set(agent_terminal);
    FEATURES
        .iter()
        .fold(Features::new(concat!(env!("CARGO_MANIFEST_DIR"), "/features")), |features, (name, define)| {
            let define = *define;
            features.feature(name, move |registry| register(registry, define))
        })
        .build_trials()
}

#[must_use]
pub fn bindings(feature: &str, step: &str) -> (usize, Option<Vec<String>>) {
    let mut registry = StepRegistry::new();
    if let Some((_, define)) = FEATURES.iter().find(|(name, _)| *name == feature) {
        register(&mut registry, *define);
    }
    (registry.matching(step).len(), registry.find(step).map(|(_, args)| args))
}
