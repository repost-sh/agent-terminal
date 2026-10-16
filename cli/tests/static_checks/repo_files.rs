use std::path::{Path, PathBuf};

pub(crate) fn cli() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

pub(crate) fn repo() -> PathBuf {
    cli().parent().expect("cli/ sits in the repository").to_path_buf()
}

pub(crate) fn files_under(dir: &Path, wanted: &dyn Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect(dir, wanted, &mut files);
    files.sort();
    files
}

fn collect(dir: &Path, wanted: &dyn Fn(&Path) -> bool, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let path = entry.expect("directory entry").path();
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        if path.is_dir() && !["target", "node_modules"].contains(&name) && !name.starts_with('.') {
            collect(&path, wanted, files);
        } else if path.is_file() && wanted(&path) {
            files.push(path);
        }
    }
}

pub(crate) fn rust_files() -> Vec<PathBuf> {
    files_under(cli(), &|path| path.extension().is_some_and(|extension| extension == "rs"))
}

pub(crate) fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).display().to_string()
}

pub(crate) fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn the_walk_finds_this_file_and_skips_build_output() {
    let files = rust_files();
    assert!(files.iter().any(|file| file.ends_with("tests/static_checks/repo_files.rs")), "{files:?}");
    let within_cli: Vec<&Path> =
        files.iter().map(|file| file.strip_prefix(cli()).expect("the walk stays under cli/")).collect();
    assert!(
        within_cli.iter().all(|file| !file.components().any(|part| part.as_os_str() == "target")),
        "{within_cli:?}"
    );
}
