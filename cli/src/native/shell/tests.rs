use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use super::{Audit, SCRIPTS, Scripts, entries, fish_quoted, read};
use crate::native::pty;
use crate::protocol::{Shell, Size};

fn command(env: &[(&str, &str)]) -> pty::Command {
    pty::Command {
        program: "shell".into(),
        args: vec!["--login".into()],
        cwd: "/".into(),
        env: env.iter().map(|(key, value)| ((*key).to_string(), (*value).to_string())).collect(),
        size: Size { cols: 80, rows: 24 },
    }
}

fn prepared(home: &Path, shell: Shell, env: &[(&str, &str)]) -> String {
    let prepared = Scripts { home }.prepare(shell, command(env)).unwrap();
    format!("{:?} {:?}", prepared.args, prepared.env).replace(&*home.to_string_lossy(), "<home>")
}

fn files(home: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut folders = vec![home.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in entries(&folder).unwrap() {
            if entry.is_dir() {
                folders.push(entry);
            } else {
                found.push(entry.strip_prefix(home).unwrap().display().to_string());
            }
        }
    }
    found.sort();
    found
}

fn audited(audit: &Audit, home: &Path) -> String {
    let relative = |paths: &[PathBuf]| -> Vec<String> {
        paths.iter().map(|path| path.strip_prefix(home).unwrap().display().to_string()).collect()
    };
    format!(
        "{} current {} outdated {:?} leftovers {:?}",
        audit.folder.strip_prefix(home).unwrap().display(),
        audit.current,
        relative(&audit.outdated),
        relative(&audit.leftovers)
    )
}

#[test]
fn zsh_starts_from_our_zdotdir_and_keeps_the_users_own_for_our_scripts_to_source() {
    let home = tempfile::tempdir().unwrap();
    let user = [("TERM", "xterm"), ("ZDOTDIR", "/home/me/.zsh"), ("AGENT_TERMINAL_ZDOTDIR", "stale")];
    assert_eq!(
        [prepared(home.path(), Shell::Zsh, &user), prepared(home.path(), Shell::Zsh, &[("TERM", "xterm")])],
        [
            r#"["-i"] [("TERM", "xterm"), ("AGENT_TERMINAL_ZDOTDIR", "/home/me/.zsh"), ("ZDOTDIR", "<home>/shell/zsh")]"#,
            r#"["-i"] [("TERM", "xterm"), ("ZDOTDIR", "<home>/shell/zsh")]"#
        ]
    );
    assert_eq!(files(home.path()), ["shell.lock", "shell/zsh/.zshenv", "shell/zsh/agent-terminal.zsh"]);
}

#[test]
fn bash_reads_our_rc_file_and_fish_sources_our_script_quoted() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(
        prepared(home.path(), Shell::Bash, &[]),
        r#"["--rcfile", "<home>/shell/bash/agent-terminal.bash", "-i"] []"#
    );
    let fish = Scripts { home: &home.path().join("it's") }.prepare(Shell::Fish, command(&[])).unwrap();
    let sourced = format!("source '{}/it\\'s/shell/fish/agent-terminal.fish'", home.path().display());
    assert_eq!(fish.args, ["--interactive", "--features", "no-mark-prompt", "--init-command", &sourced]);
    assert_eq!(fish_quoted(r"a\b'c"), r"'a\\b\'c'");
}

#[test]
fn a_script_is_written_only_when_it_changed() {
    let home = tempfile::tempdir().unwrap();
    Scripts { home: home.path() }.prepare(Shell::Bash, command(&[])).unwrap();
    let script = home.path().join("shell/bash/agent-terminal.bash");
    let inode = |path: &Path| std::fs::metadata(path).unwrap().ino();
    let written = inode(&script);
    Scripts { home: home.path() }.prepare(Shell::Bash, command(&[])).unwrap();
    assert_eq!(inode(&script), written);
    std::fs::write(&script, "edited").unwrap();
    Scripts { home: home.path() }.prepare(Shell::Bash, command(&[])).unwrap();
    let bash = SCRIPTS.iter().find(|each| each.shell == Shell::Bash).unwrap();
    assert_eq!(std::fs::read_to_string(&script).unwrap(), bash.text);
    assert_eq!(files(home.path()), ["shell.lock", "shell/bash/agent-terminal.bash"]);
}

fn every_shell_with_stray_and_stale_files(home: &Path) -> String {
    for shell in Shell::ALL {
        Scripts { home }.prepare(shell, command(&[])).unwrap();
    }
    let complete = audited(&Scripts { home }.audit().unwrap(), home);
    std::fs::write(home.join("shell/fish/agent-terminal.fish"), "old").unwrap();
    std::fs::write(home.join("shell/zsh/extra.zsh"), "").unwrap();
    std::fs::write(home.join("shell/notes"), "").unwrap();
    std::fs::create_dir_all(home.join("shell/csh/deep")).unwrap();
    complete
}

#[test]
fn an_audit_counts_current_scripts_and_names_outdated_ones_and_leftovers() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(
        audited(&Scripts { home: home.path() }.audit().unwrap(), home.path()),
        "shell current 0 outdated [] leftovers []"
    );
    let complete = every_shell_with_stray_and_stale_files(home.path());
    assert_eq!(complete, "shell current 4 outdated [] leftovers []");
    insta::assert_snapshot!(audited(&Scripts { home: home.path() }.audit().unwrap(), home.path()), @r#"shell current 3 outdated ["shell/fish/agent-terminal.fish"] leftovers ["shell/csh", "shell/notes", "shell/zsh/extra.zsh"]"#);
}

#[test]
fn a_repair_removes_the_leftovers_and_rewrites_what_is_outdated() {
    let home = tempfile::tempdir().unwrap();
    every_shell_with_stray_and_stale_files(home.path());
    let found = audited(&Scripts { home: home.path() }.audit().unwrap(), home.path());
    assert_eq!(audited(&Scripts { home: home.path() }.repair().unwrap(), home.path()), found);
    assert_eq!(
        audited(&Scripts { home: home.path() }.audit().unwrap(), home.path()),
        "shell current 4 outdated [] leftovers []"
    );
    assert_eq!(files(home.path()).len(), 5);
}

#[test]
fn a_script_reads_lossily_and_a_missing_folder_lists_nothing() {
    let home = tempfile::tempdir().unwrap();
    let odd = home.path().join("odd");
    std::fs::write(&odd, b"a\xffb").unwrap();
    assert_eq!(read(&odd).unwrap().as_deref(), Some("a\u{fffd}b"));
    assert_eq!(read(&home.path().join("missing")).unwrap(), None);
    assert_eq!(read(home.path()).unwrap_err().to_string(), format!("read {}", home.path().display()));
    assert!(entries(&home.path().join("missing")).unwrap().is_empty());
    assert_eq!(entries(&odd).unwrap_err().to_string(), format!("list {}", odd.display()));
}
