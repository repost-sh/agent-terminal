use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

pub(crate) fn bins() -> &'static Path {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let bins = Path::new(env!("CARGO_BIN_EXE_at-perf")).parent().unwrap().to_path_buf();
        let status = Command::new(env!("CARGO"))
            .args(["build", "--quiet", "-p", "agent-terminal", "-p", "at-fixtures", "--bins", "--target-dir"])
            .arg(bins.parent().unwrap())
            .status()
            .expect("build agent-terminal and the fixtures");
        assert!(status.success(), "building agent-terminal and the fixtures failed");
        bins
    })
}

pub(crate) fn at_perf(args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Output {
    Command::new(env!("CARGO_BIN_EXE_at-perf")).args(args).output().expect("run at-perf")
}

pub(crate) fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
