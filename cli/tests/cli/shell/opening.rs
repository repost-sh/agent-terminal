use std::path::{Path, PathBuf};

use super::tab::{ShellTab, timeless, which};
use crate::support::AgentTerminal;

in_every_shell!(the_users_startup_files_load_and_stay_untouched);

fn startup_files(home: &Path, shell: &str) -> Vec<(PathBuf, &'static str)> {
    match shell {
        "zsh" => vec![
            (home.join(".zshenv"), "export ZDOTDIR=\"$HOME/zdot\"\n"),
            (
                home.join("zdot/.zshrc"),
                "export FROM_RC=loaded\nalias greet='echo hello from rc'\nprecmd() { PS1='custom> ' }\n",
            ),
        ],
        "bash" => vec![(
            home.join(".bashrc"),
            "export FROM_RC=loaded\nalias greet='echo hello from rc'\nPROMPT_COMMAND='PS1=\"custom> \"'\n",
        )],
        _ => vec![(
            home.join(".config/fish/config.fish"),
            "set -gx FROM_RC loaded\nalias greet 'echo hello from rc'\nfunction fish_prompt\n    echo -n 'custom> '\nend\n",
        )],
    }
}

fn the_users_startup_files_load_and_stay_untouched(shell: &'static str) {
    let tab = ShellTab::new(shell);
    let home = tab.home.path();
    let files = startup_files(home, shell);
    for (path, text) in &files {
        std::fs::create_dir_all(path.parent().unwrap()).expect("create the config folder");
        std::fs::write(path, text).expect("write a startup file");
    }

    the_users_prompt_alias_and_variable_are_loaded(&tab);
    if shell == "zsh" {
        let zdotdir = tab.run(&["exec", "echo $ZDOTDIR"]).success().stdout();
        assert_eq!(zdotdir.lines().next(), Some(format!("{}/zdot", home.display()).as_str()));
    }
    for (path, text) in &files {
        assert_eq!(std::fs::read_to_string(path).expect("read a startup file"), *text, "{} changed", path.display());
    }
}

fn the_users_prompt_alias_and_variable_are_loaded(tab: &ShellTab) {
    let opened = tab.open().success().stdout();
    assert!(opened.contains("│custom>\n"), "the user's prompt is not on screen:\n{opened}");
    let cwd = tab.cwd();
    assert_eq!(
        timeless(&tab.run(&["exec", "greet"]).success().stdout()),
        format!("hello from rc\n1 · exit 0 · <time> · {cwd} · greet\n")
    );
    assert_eq!(
        timeless(&tab.run(&["exec", "echo $FROM_RC"]).success().stdout()),
        format!("loaded\n2 · exit 0 · <time> · {cwd} · echo $FROM_RC\n")
    );
}

#[test]
fn a_tab_without_shell_integration_says_so() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "20x2", "cat"]).success();
    let refusal = "error: work app runs cat without shell integration, so it keeps no command history; exec, history and wait --command work in a tab opened with `agent-terminal shell`\n";
    for args in [&["history"][..], &["history", "show", "1"], &["exec", "ls"], &["wait", "--command"]] {
        let run = at.run(args);
        assert_eq!(run.code(), 1, "{args:?}");
        assert_eq!(run.stderr(), refusal, "{args:?}");
    }
}

#[test]
fn restart_opens_the_shell_again_with_integration() {
    let tab = ShellTab::opened("zsh");
    tab.run(&["exec", "echo before"]).success();
    tab.run(&["restart"]).success();
    let cwd = tab.cwd();
    assert_eq!(
        timeless(&tab.run(&["exec", "echo after"]).success().stdout()),
        format!("after\n2 · exit 0 · <time> · {cwd} · echo after\n")
    );
}

#[test]
fn shell_says_when_the_first_prompt_never_comes() {
    let tab = ShellTab::new("bash");
    std::fs::write(tab.home.path().join(".bashrc"), "echo starting\nread -r answer\n").expect("write .bashrc");
    let run = tab.run(&["shell", "--name", "shell", "--shell", "bash", "--size", "40x5", "--timeout", "500"]);
    assert_eq!(run.code(), 3);
    assert_eq!(
        run.stderr().lines().next(),
        Some(
            "error: timed out after 500 ms waiting for the first prompt of bash in work shell; its startup files may be waiting for input or the shell integration did not load (read the screen with `agent-terminal snapshot`)"
        )
    );
    assert!(run.stderr().contains("│starting\n"), "{}", run.stderr());
}

#[test]
fn shell_opens_the_shell_your_shell_variable_names() {
    let tab = ShellTab::new("bash");
    let work = tab.work.path().to_string_lossy().into_owned();
    let shell_args = ["shell", "--name", "shell", "--size", "40x5", "--cwd", work.as_str()];
    let bash = which("bash");
    tab.run_with(&shell_args, &[("SHELL", Some(bash.as_str()))]).success();
    assert_eq!(tab.run(&["get", "program"]).success().stdout(), "bash\n");
    tab.run(&["exec", "true"]).success();

    let other = tab.run_with(&shell_args, &[("SHELL", Some("/bin/tcsh"))]);
    assert_eq!(other.code(), 1);
    assert_eq!(
        other.stderr(),
        "error: your SHELL is /bin/tcsh, which has no shell integration; choose --shell zsh, --shell bash or --shell fish\n"
    );
    let unset = tab.run_with(&shell_args, &[("SHELL", None)]);
    assert_eq!(unset.code(), 1);
    assert_eq!(
        unset.stderr(),
        "error: SHELL is not set, so agent-terminal does not know your shell; choose one with --shell zsh, --shell bash or --shell fish\n"
    );
}
