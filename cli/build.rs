use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo::rerun-if-changed=src");
    println!("cargo::rerun-if-changed=Cargo.toml");
    println!("cargo::rerun-if-changed=build.rs");
    if let Some(git_dir) = git(&["rev-parse", "--git-dir"]) {
        let git_dir = Path::new(&git_dir);
        println!("cargo::rerun-if-changed={}", git_dir.join("HEAD").display());
        println!("cargo::rerun-if-changed={}", git_dir.join("index").display());
    }
    println!("cargo::rustc-env=AGENT_TERMINAL_COMMIT={}", commit());
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn commit() -> String {
    let Some(head) = git(&["rev-parse", "--short=12", "HEAD"]) else {
        return "unknown".to_string();
    };
    match git(&["status", "--porcelain", "--untracked-files=no"]) {
        Some(changes) if changes.is_empty() => head,
        _ => format!("{head}.dirty"),
    }
}
