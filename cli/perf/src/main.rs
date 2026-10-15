mod check;
mod load;
mod measure;
mod metrics;

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};

use crate::measure::Setup;
use crate::metrics::Results;

const USAGE: &str = "usage: at-perf measure --base <agent-terminal> --change <agent-terminal> --fixtures <dir> --work <empty dir> --out <results.json>\n       at-perf check <results.json> [--summary <file>]\n       at-perf load --agent <agent-terminal> --fixtures <dir> --work <empty dir> --out <rows.json> [--seconds 10] [--rates 1,30,60,120] [--tabs 1,10,20] [--repaints partial,full] [--viewers none,live,dashboard]";

const FAILED: u8 = 2;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("measure") => measure(args),
        Some("check") => check(args),
        Some("load") => load(args),
        _ => bail!("{USAGE}"),
    }
}

fn measure(mut args: impl Iterator<Item = String>) -> Result<ExitCode> {
    let (mut base, mut change, mut fixtures, mut work, mut out) = (None, None, None, None, None);
    while let Some(arg) = args.next() {
        let slot = match arg.as_str() {
            "--base" => &mut base,
            "--change" => &mut change,
            "--fixtures" => &mut fixtures,
            "--work" => &mut work,
            "--out" => &mut out,
            other => bail!("unexpected argument {other}\n{USAGE}"),
        };
        *slot = Some(PathBuf::from(args.next().with_context(|| format!("{arg} needs a path"))?));
    }
    let missing = |name: &str| format!("{name} is missing\n{USAGE}");
    let setup = Setup {
        base: base.with_context(|| missing("--base"))?,
        change: change.with_context(|| missing("--change"))?,
        fixtures: fixtures.with_context(|| missing("--fixtures"))?,
        work: work.with_context(|| missing("--work"))?,
    };
    let out = out.with_context(|| missing("--out"))?;
    let results = measure::measure(&setup)?;
    let json = serde_json::to_string_pretty(&results).context("serialize the results")?;
    std::fs::write(&out, json).with_context(|| format!("write {}", out.display()))?;
    Ok(ExitCode::SUCCESS)
}

fn load(mut args: impl Iterator<Item = String>) -> Result<ExitCode> {
    let mut paths: [Option<PathBuf>; 4] = Default::default();
    let mut plan = load::Plan {
        agent: PathBuf::new(),
        fixtures: PathBuf::new(),
        work: PathBuf::new(),
        seconds: 10,
        rates: vec![1, 30, 60, 120],
        tabs: vec![1, 10, 20],
        repaints: vec![load::Repaint::Partial, load::Repaint::Full],
        viewers: vec![load::Viewer::None, load::Viewer::Live, load::Viewer::Dashboard],
    };
    while let Some(arg) = args.next() {
        let value = args.next().with_context(|| format!("{arg} needs a value"))?;
        match arg.as_str() {
            "--agent" => paths[0] = Some(PathBuf::from(value)),
            "--fixtures" => paths[1] = Some(PathBuf::from(value)),
            "--work" => paths[2] = Some(PathBuf::from(value)),
            "--out" => paths[3] = Some(PathBuf::from(value)),
            "--seconds" => plan.seconds = value.parse().context("--seconds needs a whole number")?,
            "--rates" => plan.rates = list(&value, |rate| rate.parse().context("--rates needs whole numbers"))?,
            "--tabs" => plan.tabs = list(&value, |tabs| tabs.parse().context("--tabs needs whole numbers"))?,
            "--repaints" => plan.repaints = list(&value, load::Repaint::parse)?,
            "--viewers" => plan.viewers = list(&value, load::Viewer::parse)?,
            other => bail!("unexpected argument {other}\n{USAGE}"),
        }
    }
    let [agent, fixtures, work, out] = paths;
    let missing = |name: &str| format!("{name} is missing\n{USAGE}");
    plan.agent = agent.with_context(|| missing("--agent"))?;
    plan.fixtures = fixtures.with_context(|| missing("--fixtures"))?;
    plan.work = work.with_context(|| missing("--work"))?;
    let out = out.with_context(|| missing("--out"))?;
    eprintln!("{}", load::HEADER);
    let rows = load::run(&plan)?;
    std::fs::write(&out, serde_json::to_string_pretty(&rows).context("serialize the rows")?)
        .with_context(|| format!("write {}", out.display()))?;
    println!("{}", load::HEADER);
    for row in &rows {
        println!("{}", load::line(row));
    }
    Ok(ExitCode::SUCCESS)
}

fn list<T>(text: &str, parse: impl Fn(&str) -> Result<T>) -> Result<Vec<T>> {
    text.split(',').map(|item| parse(item.trim())).collect()
}

fn check(mut args: impl Iterator<Item = String>) -> Result<ExitCode> {
    let mut results = None;
    let mut summary = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--summary" => summary = Some(PathBuf::from(args.next().context("--summary needs a file")?)),
            path if results.is_none() => results = Some(PathBuf::from(path)),
            other => bail!("unexpected argument {other}\n{USAGE}"),
        }
    }
    let path = results.context(USAGE)?;
    let json = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let results: Results = serde_json::from_str(&json).with_context(|| format!("parse {}", path.display()))?;
    let report = check::check(&results)?;
    print!("{}", report.text);
    if let Some(summary) = summary {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&summary)
            .and_then(|mut file| file.write_all(report.text.as_bytes()))
            .with_context(|| format!("append the report to {}", summary.display()))?;
    }
    Ok(if report.passed { ExitCode::SUCCESS } else { ExitCode::from(FAILED) })
}
