mod coverage;
mod crap;
mod functions;
mod lcov;
mod offenses;
mod workspace;

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};

use crate::offenses::Verdict;

const USAGE: &str = "usage: at-gauntlet coverage <lcov.info>\n       at-gauntlet crap <lcov.info>";

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
    let [gate, report] = args else { bail!("{USAGE}") };
    let check = match gate.as_str() {
        "coverage" => coverage::check,
        "crap" => crap::check,
        other => bail!("unknown gate {other}\n{USAGE}"),
    };
    let lines = lcov::read(Path::new(report))?;
    let workspace = workspace::Workspace::load().context("read the workspace with cargo metadata")?;
    check(&workspace, &lines)
}
