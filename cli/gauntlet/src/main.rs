mod architecture;
mod coverage;
mod crap;
mod gherkin;
mod lcov;
mod mutation;
mod offenses;
mod params;
mod properties;
mod rust;
mod workspace;

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};

use crate::offenses::Verdict;

const USAGE: &str = "usage: at-gauntlet coverage <lcov.info>\n       at-gauntlet crap <lcov.info>\n       at-gauntlet props\n       at-gauntlet architecture\n       at-gauntlet params\n       at-gauntlet gherkin\n       at-gauntlet gherkin-ir\n       at-gauntlet mutation <outcomes.json>...\n       at-gauntlet faults <outcomes.json> <regex>...";

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
    match args {
        [gate, reports @ ..] if gate == "mutation" && !reports.is_empty() => {
            mutation::check(&reports.iter().map(Path::new).collect::<Vec<_>>())
        }
        [gate, report, faults @ ..] if gate == "faults" && !faults.is_empty() => {
            mutation::check_faults(Path::new(report), &faults.iter().map(String::as_str).collect::<Vec<_>>())
        }
        [gate] => unmeasured(gate)?(&load()?),
        [gate, report] => {
            let check = measured(gate)?;
            let lines = lcov::read(Path::new(report))?;
            check(&load()?, &lines)
        }
        _ => bail!("{USAGE}"),
    }
}

type MeasuredCheck = fn(&workspace::Workspace, &lcov::Lines) -> Result<Verdict>;

type StaticCheck = fn(&workspace::Workspace) -> Result<Verdict>;

fn measured(gate: &str) -> Result<MeasuredCheck> {
    match gate {
        "coverage" => Ok(coverage::check),
        "crap" => Ok(crap::check),
        other => bail!("unknown gate {other}\n{USAGE}"),
    }
}

fn unmeasured(gate: &str) -> Result<StaticCheck> {
    match gate {
        "props" => Ok(properties::check),
        "architecture" => Ok(architecture::check),
        "params" => Ok(params::check),
        "gherkin" => Ok(gherkin::check),
        "gherkin-ir" => Ok(gherkin::write_ir),
        _ => bail!("{USAGE}"),
    }
}

fn load() -> Result<workspace::Workspace> {
    workspace::Workspace::load().context("read the workspace with cargo metadata")
}
