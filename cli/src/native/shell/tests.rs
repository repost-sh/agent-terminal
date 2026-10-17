use std::os::unix::fs::MetadataExt;
use std::path::Path;

use super::{SCRIPTS, Scripts, fish_quoted, read};
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
        for entry in std::fs::read_dir(&folder).unwrap() {
            let entry = entry.unwrap().path();
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

#[test]
fn a_script_reads_lossily_and_a_missing_one_reads_as_nothing() {
    let home = tempfile::tempdir().unwrap();
    let odd = home.path().join("odd");
    std::fs::write(&odd, b"a\xffb").unwrap();
    assert_eq!(read(&odd).unwrap().as_deref(), Some("a\u{fffd}b"));
    assert_eq!(read(&home.path().join("missing")).unwrap(), None);
    assert_eq!(read(home.path()).unwrap_err().to_string(), format!("read {}", home.path().display()));
}
