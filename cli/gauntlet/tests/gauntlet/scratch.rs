use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) struct Run {
    pub(crate) code: i32,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

pub(crate) struct Scratch {
    dir: tempfile::TempDir,
}

impl Scratch {
    pub(crate) fn crate_with(files: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().expect("scratch workspace");
        write(&dir.path().join("Cargo.toml"), "[workspace]\nmembers = [\"app\"]\nresolver = \"3\"\n");
        write(
            &dir.path().join("app/Cargo.toml"),
            "[package]\nname = \"app\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        );
        for (path, code) in files {
            write(&dir.path().join("app/src").join(path), code);
        }
        Self { dir }
    }

    pub(crate) fn with(self, path: &str, code: &str) -> Self {
        write(&self.dir.path().join("app").join(path), code);
        self
    }

    pub(crate) fn root(&self) -> PathBuf {
        self.dir.path().to_path_buf()
    }

    pub(crate) fn path(&self, path: &str) -> PathBuf {
        self.dir.path().join("app").join(path)
    }

    pub(crate) fn source(&self, path: &str) -> PathBuf {
        std::fs::canonicalize(self.dir.path().join("app/src").join(path)).expect("a source file")
    }

    pub(crate) fn props(&self) -> Run {
        run(self.dir.path(), &["props"])
    }

    pub(crate) fn gauntlet(&self, gate: &str, report: &str) -> Run {
        let path = self.dir.path().join("lcov.info");
        write(&path, report);
        run(self.dir.path(), &[gate, path.to_str().expect("a UTF-8 path")])
    }
}

pub(crate) fn run(dir: &Path, args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_at-gauntlet"))
        .args(args)
        .current_dir(dir)
        .env("CARGO", env!("CARGO"))
        .output()
        .expect("run at-gauntlet");
    Run {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("a parent folder")).expect("create the folder");
    std::fs::write(path, text).expect("write the file");
}

pub(crate) fn lcov(records: &[(&Path, &[(u32, u64)])]) -> String {
    let mut report = String::new();
    for (file, hits) in records {
        writeln!(report, "TN:\nSF:{}", file.display()).expect("write the report");
        for (line, hit) in *hits {
            writeln!(report, "DA:{line},{hit}").expect("write the report");
        }
        report.push_str("end_of_record\n");
    }
    report
}

pub(crate) fn hits(lines: std::ops::RangeInclusive<u32>, covered: impl Fn(u32) -> bool) -> Vec<(u32, u64)> {
    lines.map(|line| (line, u64::from(covered(line)))).collect()
}
