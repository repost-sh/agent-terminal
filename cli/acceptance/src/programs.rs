use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

static AGENT_TERMINAL: OnceLock<PathBuf> = OnceLock::new();
static FIXTURES: OnceLock<Result<PathBuf, String>> = OnceLock::new();

pub(crate) fn set(agent_terminal: &Path) {
    AGENT_TERMINAL.get_or_init(|| agent_terminal.to_path_buf());
}

pub(crate) fn agent_terminal() -> Result<&'static Path, String> {
    AGENT_TERMINAL
        .get()
        .map(PathBuf::as_path)
        .ok_or_else(|| "trials() was not given the agent-terminal binary".to_string())
}

pub(crate) fn fixture(name: &str) -> Result<PathBuf, String> {
    let built = FIXTURES.get_or_init(build_fixtures).as_ref().map_err(Clone::clone)?;
    Ok(built.join(name))
}

fn build_fixtures() -> Result<PathBuf, String> {
    let bins = agent_terminal()?.parent().ok_or("the agent-terminal binary sits in a folder")?;
    let target = bins.parent().ok_or("the binaries folder sits in a target folder")?;
    let built = Command::new(env!("CARGO"))
        .args(["build", "--quiet", "-p", "at-fixtures", "--bins", "--target-dir"])
        .arg(target)
        .status()
        .map_err(|e| format!("build the fixture programs: {e}"))?;
    if built.success() { Ok(bins.to_path_buf()) } else { Err(format!("building the fixture programs failed: {built}")) }
}
