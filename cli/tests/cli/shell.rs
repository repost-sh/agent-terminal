use std::path::{Path, PathBuf};

use crate::support::{AgentTerminal, Run};

struct ShellTab {
    at: AgentTerminal,
    shell: &'static str,
    home: tempfile::TempDir,
    work: tempfile::TempDir,
}

impl ShellTab {
    fn new(shell: &'static str) -> Option<Self> {
        if !installed(shell) {
            eprintln!("skipping the {shell} case: {shell} is not installed (no {shell} on PATH)");
            return None;
        }
        let temp = |prefix: &str| tempfile::Builder::new().prefix(prefix).tempdir_in("/tmp").expect("temp folder");
        Some(Self { at: AgentTerminal::new(), shell, home: temp("home-"), work: temp("work-") })
    }

    fn opened(shell: &'static str) -> Option<Self> {
        let tab = Self::new(shell)?;
        tab.open().success();
        Some(tab)
    }

    fn open(&self) -> Run {
        let work = self.work.path().to_string_lossy().into_owned();
        self.run(&["shell", "--shell", self.shell, "--size", "60x10", "--cwd", &work])
    }

    fn run(&self, args: &[&str]) -> Run {
        self.run_with(args, &[])
    }

    fn run_with(&self, args: &[&str], env: &[(&str, Option<&str>)]) -> Run {
        let mut command = self.at.command(args);
        command.env("HOME", self.home.path()).env_remove("ZDOTDIR");
        for (key, value) in env {
            match value {
                Some(value) => command.env(key, value),
                None => command.env_remove(key),
            };
        }
        Run(command.output().expect("run agent-terminal"))
    }

    fn cwd(&self) -> String {
        canonical(self.work.path())
    }
}

fn canonical(path: &Path) -> String {
    std::fs::canonicalize(path).expect("canonical path").to_string_lossy().into_owned()
}

fn installed(shell: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir: PathBuf| dir.join(shell).is_file()))
}

fn timeless(text: &str) -> String {
    let duration = regex::Regex::new(r" · (\d+ ms|\d+\.\d s|\d+ min \d{2} s) · ").unwrap();
    duration.replace_all(text, " · <time> · ").into_owned()
}

macro_rules! in_every_shell {
    ($($scenario:ident),* $(,)?) => {
        $(mod $scenario {
            #[test]
            fn zsh() {
                super::$scenario("zsh")
            }

            #[test]
            fn bash() {
                super::$scenario("bash")
            }

            #[test]
            fn fish() {
                super::$scenario("fish")
            }
        })*
    };
}

in_every_shell!(
    exec_prints_the_output_and_the_exit_code,
    history_keeps_every_command_with_its_exit_code_cwd_and_output,
    wait_command_waits_for_a_long_running_command,
    the_users_startup_files_load_and_stay_untouched,
);

fn exec_prints_the_output_and_the_exit_code(shell: &'static str) {
    let Some(tab) = ShellTab::opened(shell) else { return };
    let cwd = tab.cwd();
    let run = tab.run(&["exec", "echo hello"]).success();
    assert_eq!(timeless(&run.stdout()), format!("hello\n1 · exit 0 · <time> · {cwd} · echo hello\n"));
}

fn history_keeps_every_command_with_its_exit_code_cwd_and_output(shell: &'static str) {
    let Some(tab) = ShellTab::opened(shell) else { return };
    let cwd = tab.cwd();
    let sub = format!("{cwd}/sub");
    assert_eq!(tab.run(&["history"]).success().stdout(), "no commands have run in default t1\n");
    tab.run(&["exec", "true"]).success();
    assert_eq!(tab.run(&["history", "--failed"]).success().stdout(), "no command matches in default t1 (1 in all)\n");
    tab.run(&["restart"]).success();

    let failed = tab.run(&["exec", "false"]).success();
    assert_eq!(timeless(&failed.stdout()), format!("1 · exit 1 · <time> · {cwd} · false\n"));
    let checked = tab.run(&["exec", "--check", "sh -c 'echo to stderr >&2; exit 3'"]);
    assert_eq!(checked.code(), 3, "{}", checked.stderr());
    assert_eq!(
        timeless(&checked.stdout()),
        format!("to stderr\n2 · exit 3 · <time> · {cwd} · sh -c 'echo to stderr >&2; exit 3'\n")
    );
    tab.run(&["exec", "--check", "mkdir sub && cd sub"]).success();
    let numbers: String = (1..=25).map(|n| format!("{n}\n")).collect();
    let scrolled = tab.run(&["exec", "seq 1 25"]).success();
    assert_eq!(timeless(&scrolled.stdout()), format!("{numbers}4 · exit 0 · <time> · {sub} · seq 1 25\n"));

    let all = format!(
        "1 · exit 1 · <time> · {cwd} · false\n\
         2 · exit 3 · <time> · {cwd} · sh -c 'echo to stderr >&2; exit 3'\n\
         3 · exit 0 · <time> · {cwd} · mkdir sub && cd sub\n\
         4 · exit 0 · <time> · {sub} · seq 1 25\n"
    );
    let lines: Vec<&str> = all.lines().collect();
    assert_eq!(timeless(&tab.run(&["history"]).success().stdout()), all);
    assert_eq!(
        timeless(&tab.run(&["history", "--failed"]).success().stdout()),
        format!("{}\n{}\n", lines[0], lines[1])
    );
    assert_eq!(timeless(&tab.run(&["history", "--last", "1"]).success().stdout()), format!("{}\n", lines[3]));
    assert_eq!(
        timeless(&tab.run(&["history", "--failed", "--last", "1"]).success().stdout()),
        format!("{}\n", lines[1])
    );
    assert_eq!(timeless(&tab.run(&["history", "show", "4"]).success().stdout()), format!("{numbers}{}\n", lines[3]));
    let missing = tab.run(&["history", "show", "9"]);
    assert_eq!(missing.code(), 1);
    assert_eq!(missing.stderr(), "error: no command 9 in default t1; list them with `agent-terminal history`\n");
    assert_eq!(tab.run(&["get", "cwd"]).success().stdout(), format!("{sub}\n"));
}

fn wait_command_waits_for_a_long_running_command(shell: &'static str) {
    let Some(tab) = ShellTab::opened(shell) else { return };
    let cwd = tab.cwd();
    let hold = tab.work.path().join("hold");
    let status = std::process::Command::new("mkfifo").arg(&hold).status().expect("run mkfifo");
    assert!(status.success(), "mkfifo failed");
    let waited = regex::Regex::new(r"^waited \d+ ms ").unwrap();
    let at_prompt = tab.run(&["wait", "--command"]).success().stdout();
    assert_eq!(
        waited.replace(&at_prompt, "waited N ms "),
        "waited N ms for the shell command to finish in default t1\n"
    );

    let line = "cat hold; echo finished";
    let cut = tab.run(&["exec", line, "--timeout", "300"]);
    assert_eq!(cut.code(), 3);
    assert_eq!(
        cut.stderr().lines().next(),
        Some(
            "error: timed out after 300 ms waiting for `cat hold; echo finished` to finish in default t1; it is still running as command 1 (wait for it with `agent-terminal wait --command`, read its output with `agent-terminal history show 1`, interrupt it with `agent-terminal signal INT`)"
        )
    );
    assert_eq!(timeless(&tab.run(&["history"]).success().stdout()), format!("1 · running · <time> · {cwd} · {line}\n"));
    assert_eq!(tab.run(&["wait", "--command", "--timeout", "300"]).code(), 3);
    let queued = tab.run(&["exec", "echo next", "--timeout", "300"]);
    assert_eq!(queued.code(), 3);
    assert_eq!(
        queued.stderr().lines().next(),
        Some(
            "error: timed out after 300 ms waiting for `cat hold; echo finished` to finish in default t1, so `echo next` did not run (read the screen with `agent-terminal snapshot`, interrupt a command with `agent-terminal signal INT`)"
        )
    );

    std::fs::write(&hold, "released\n").expect("release the command");
    let finished = tab.run(&["wait", "--command"]).success().stdout();
    assert_eq!(
        waited.replace(&finished, "waited N ms "),
        "waited N ms for the shell command to finish in default t1 (exit code 0)\n"
    );
    assert_eq!(
        timeless(&tab.run(&["history", "show", "1"]).success().stdout()),
        format!("released\nfinished\n1 · exit 0 · <time> · {cwd} · {line}\n")
    );

    tab.run(&["type", "cat hold\n"]).success();
    assert_eq!(tab.run(&["wait", "--command", "--timeout", "300"]).code(), 3);
    std::fs::write(&hold, "typed\n").expect("release the typed command");
    tab.run(&["wait", "--command"]).success();
    assert_eq!(
        timeless(&tab.run(&["history", "show", "2"]).success().stdout()),
        format!("typed\n2 · exit 0 · <time> · {cwd} · cat hold\n")
    );
}

fn the_users_startup_files_load_and_stay_untouched(shell: &'static str) {
    let Some(tab) = ShellTab::new(shell) else { return };
    let home = tab.home.path();
    let files: Vec<(PathBuf, &str)> = match shell {
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
    };
    for (path, text) in &files {
        std::fs::create_dir_all(path.parent().unwrap()).expect("create the config folder");
        std::fs::write(path, text).expect("write a startup file");
    }

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
    if shell == "zsh" {
        let zdotdir = tab.run(&["exec", "echo $ZDOTDIR"]).success().stdout();
        assert_eq!(zdotdir.lines().next(), Some(format!("{}/zdot", home.display()).as_str()));
    }
    for (path, text) in &files {
        assert_eq!(std::fs::read_to_string(path).expect("read a startup file"), *text, "{} changed", path.display());
    }
}

#[test]
fn a_tab_without_shell_integration_says_so() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x2", "cat"]).success();
    let refusal = "error: default t1 runs cat without shell integration, so it keeps no command history; exec, history and wait --command work in a tab opened with `agent-terminal shell`\n";
    for args in [&["history"][..], &["history", "show", "1"], &["exec", "ls"], &["wait", "--command"]] {
        let run = at.run(args);
        assert_eq!(run.code(), 1, "{args:?}");
        assert_eq!(run.stderr(), refusal, "{args:?}");
    }
}

#[test]
fn exec_says_when_the_shell_exits() {
    let Some(tab) = ShellTab::opened("zsh") else { return };
    let run = tab.run(&["exec", "exit 4"]);
    assert_eq!(run.code(), 5);
    assert_eq!(
        run.stderr().lines().next(),
        Some("error: stopped waiting for the shell command to finish: zsh in default t1 has exited with code 4")
    );
    assert_eq!(tab.run(&["wait", "--command"]).code(), 5);
    assert_eq!(tab.run(&["exec", "echo late"]).code(), 5);
}

#[test]
fn exec_refuses_what_is_not_one_command_line() {
    let Some(tab) = ShellTab::opened("zsh") else { return };
    let empty = tab.run(&["exec", " "]);
    assert_eq!(empty.code(), 1);
    assert_eq!(empty.stderr(), "error: exec needs a command line, e.g. exec \"cargo test\"\n");
    let lines = tab.run(&["exec", "echo a\necho b"]);
    assert_eq!(lines.code(), 1);
    assert_eq!(lines.stderr(), "error: exec runs one command line; join commands with ; or && instead of newlines\n");
    let unparsed = tab.run(&["exec", ")"]);
    assert_eq!(unparsed.code(), 1);
    let stderr = unparsed.stderr();
    assert_eq!(
        stderr.lines().next(),
        Some("error: the shell in default t1 ran no command for `)`; the screen shows why")
    );
    assert!(stderr.contains("parse error"), "{stderr}");
    assert_eq!(tab.run(&["history"]).success().stdout(), "no commands have run in default t1\n");
}

#[test]
fn exec_and_history_print_json() {
    let Some(tab) = ShellTab::opened("bash") else { return };
    let cwd = tab.cwd();
    let mut ran = tab.run(&["--json", "exec", "echo hi"]).success().json();
    let started = ran["started_at_ms"].as_u64().expect("started_at_ms");
    let ended = ran["ended_at_ms"].as_u64().expect("ended_at_ms");
    assert!(started <= ended, "{ran}");
    let first_frame = ran["first_frame"].as_u64().expect("first_frame");
    assert!(ran["last_frame"].as_u64().expect("last_frame") >= first_frame, "{ran}");
    ran["duration_ms"].as_u64().expect("duration_ms");
    let masked = serde_json::json!("<masked>");
    for timing in ["started_at_ms", "ended_at_ms", "duration_ms", "first_frame", "last_frame"] {
        ran[timing] = masked.clone();
    }
    assert_eq!(
        ran,
        serde_json::json!({
            "session": "default", "tab": "t1", "id": 1, "command": "echo hi", "cwd": cwd,
            "started_at_ms": masked, "ended_at_ms": masked, "duration_ms": masked, "exit_code": 0,
            "first_frame": masked, "last_frame": masked, "output": "hi\n"
        })
    );
    let checked = tab.run(&["--json", "exec", "--check", "false"]);
    assert_eq!(checked.code(), 1);
    assert_eq!(checked.json()["check"], true);
    assert_eq!(checked.json()["exit_code"], 1);

    let history = tab.run(&["--json", "history", "--failed"]).success().json();
    assert_eq!(history["session"], "default");
    assert_eq!(history["tab"], "t1");
    let commands = history["commands"].as_array().expect("commands");
    assert_eq!(commands.len(), 1, "{history}");
    assert_eq!(commands[0]["id"], 2);
    assert_eq!(commands[0]["command"], "false");
    assert_eq!(commands[0]["exit_code"], 1);
    assert!(commands[0].get("output").is_none(), "history lists commands without their output: {history}");
}

#[test]
fn restart_opens_the_shell_again_with_integration() {
    let Some(tab) = ShellTab::opened("zsh") else { return };
    tab.run(&["exec", "echo before"]).success();
    tab.run(&["restart"]).success();
    let cwd = tab.cwd();
    assert_eq!(
        timeless(&tab.run(&["exec", "echo after"]).success().stdout()),
        format!("after\n1 · exit 0 · <time> · {cwd} · echo after\n")
    );
}

#[test]
fn shell_says_when_the_first_prompt_never_comes() {
    let Some(tab) = ShellTab::new("bash") else { return };
    std::fs::write(tab.home.path().join(".bashrc"), "echo starting\nread -r answer\n").expect("write .bashrc");
    let run = tab.run(&["shell", "--shell", "bash", "--size", "40x5", "--timeout", "500"]);
    assert_eq!(run.code(), 3);
    assert_eq!(
        run.stderr().lines().next(),
        Some(
            "error: timed out after 500 ms waiting for the first prompt of bash in default t1; its startup files may be waiting for input or the shell integration did not load (read the screen with `agent-terminal snapshot`, check with `agent-terminal doctor`)"
        )
    );
    assert!(run.stderr().contains("│starting\n"), "{}", run.stderr());
}

#[test]
fn execs_sent_together_run_one_after_the_other() {
    let Some(tab) = ShellTab::opened("zsh") else { return };
    let words = ["alpha", "bravo", "charlie", "delta"];
    let outputs: Vec<String> = std::thread::scope(|scope| {
        let calls: Vec<_> = words
            .iter()
            .map(|word| {
                let line = format!("echo {word}");
                let tab = &tab;
                scope.spawn(move || tab.run(&["exec", &line]).success().stdout())
            })
            .collect();
        calls.into_iter().map(|call| call.join().unwrap()).collect()
    });
    let mut ids = Vec::new();
    for (word, output) in words.iter().zip(&outputs) {
        let (printed, record) = output.split_once('\n').expect("output, then the record line");
        assert_eq!(printed, *word, "{output}");
        assert!(record.ends_with(&format!(" · echo {word}\n")), "{output}");
        ids.push(record.split(' ').next().unwrap().to_string());
    }
    ids.sort();
    assert_eq!(ids, ["1", "2", "3", "4"]);
    assert_eq!(tab.run(&["history"]).success().stdout().lines().count(), 4);
}

#[test]
fn shell_opens_the_shell_your_shell_variable_names() {
    let Some(tab) = ShellTab::new("bash") else { return };
    let work = tab.work.path().to_string_lossy().into_owned();
    let shell_args = ["shell", "--size", "40x5", "--cwd", work.as_str()];
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

fn which(program: &str) -> String {
    let path = std::env::var_os("PATH").expect("PATH");
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| panic!("{program} is not on PATH"))
        .to_string_lossy()
        .into_owned()
}
