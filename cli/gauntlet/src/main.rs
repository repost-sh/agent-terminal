mod coverage;
mod crap;
mod lcov;
mod offenses;
mod properties;
mod rust;
mod workspace;

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};

use crate::offenses::Verdict;

const USAGE: &str =
    "usage: at-gauntlet coverage <lcov.info>\n       at-gauntlet crap <lcov.info>\n       at-gauntlet props";

const FAILED: u8 = 2;

fn main() -> ExitCode {
    let verdict = run(&std::env::args().skip(1).collect::<Vec<_>>());
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    let written = match &verdict {
        Ok(verdict) => verdict.lines.iter().try_for_each(|line| writeln!(stdout, "{line}")),
        Err(e) => writeln!(stderr, "error: {e:#}"),
    };
    match (verdict, written) {
        (Ok(verdict), Ok(())) if verdict.passed => ExitCode::SUCCESS,
        (Ok(_), Ok(())) => ExitCode::from(FAILED),
        _ => ExitCode::FAILURE,
    }
}

fn run(args: &[String]) -> Result<Verdict> {
    let gate = match args {
        [gate] if gate == "props" => Gate::Props,
        [gate, report] => Gate::Measured { check: measured(gate)?, report: Path::new(report) },
        _ => bail!("{USAGE}"),
    };
    match gate {
        Gate::Props => properties::check(&load()?),
        Gate::Measured { check, report } => {
            let lines = lcov::read(report)?;
            check(&load()?, &lines)
        }
    }
}

type MeasuredCheck = fn(&workspace::Workspace, &lcov::Lines) -> Result<Verdict>;

enum Gate<'a> {
    Props,
    Measured { check: MeasuredCheck, report: &'a Path },
}

fn measured(gate: &str) -> Result<MeasuredCheck> {
    match gate {
        "coverage" => Ok(coverage::check),
        "crap" => Ok(crap::check),
        other => bail!("unknown gate {other}\n{USAGE}"),
    }
}

fn load() -> Result<workspace::Workspace> {
    workspace::Workspace::load().context("read the workspace with cargo metadata")
}
