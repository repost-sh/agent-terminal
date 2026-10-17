use std::path::{Path, PathBuf};
use std::process::Command;

const INPUTS: [&str; 3] = [".", "../skill-data", "../skills"];

fn main() {
    println!("cargo::rerun-if-changed=src");
    println!("cargo::rerun-if-changed=Cargo.toml");
    println!("cargo::rerun-if-changed=build.rs");
    for file in head_files() {
        println!("cargo::rerun-if-changed={}", file.display());
    }
    println!("cargo::rustc-env=AGENT_TERMINAL_COMMIT={}", commit());
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").arg("--no-optional-locks").args(args).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn head_files() -> Vec<PathBuf> {
    let (Some(git_dir), Some(common_dir)) = (git(&["rev-parse", "--git-dir"]), git(&["rev-parse", "--git-common-dir"]))
    else {
        return Vec::new();
    };
    let common_dir = Path::new(&common_dir);
    let mut files = vec![Path::new(&git_dir).join("HEAD")];
    files.extend(git(&["symbolic-ref", "-q", "HEAD"]).map(|branch| common_dir.join(branch)));
    files.extend(Some(common_dir.join("packed-refs")).filter(|packed| packed.exists()));
    files
}

fn commit() -> String {
    let Some(head) = git(&["rev-parse", "--short=12", "HEAD"]) else {
        return "unknown".to_string();
    };
    let status = [&["status", "--porcelain", "--untracked-files=no", "--"][..], &INPUTS].concat();
    match git(&status) {
        Some(changes) if changes.is_empty() => head,
        _ => format!("{head}.dirty"),
    }
}
