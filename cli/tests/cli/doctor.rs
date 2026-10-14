use crate::support::{AgentTerminal, Run};

const NO_SCRIPTS_YET: &str = "ok       shell    zsh (your SHELL) has shell integration; `agent-terminal shell` writes its scripts when it first runs\n";

fn doctor(at: &AgentTerminal, args: &[&str]) -> Run {
    at.run_with(args, "", &[("SHELL", "/bin/zsh")])
}

#[test]
fn doctor_checks_the_pty_and_the_daemon_without_starting_it() {
    let at = AgentTerminal::new();
    let socket = at.home().join("daemon.sock");
    let run = doctor(&at, &["doctor"]).success();
    assert_eq!(
        run.stdout(),
        format!(
            "ok       pty      opened a pseudo-terminal\n{NO_SCRIPTS_YET}ok       daemon   not running; the next command starts it\n"
        )
    );
    assert!(!socket.exists(), "doctor must not start the daemon");

    at.run(&["open", "--size", "20x2", "cat"]).success();
    let run = doctor(&at, &["doctor"]).success();
    assert_eq!(
        run.stdout(),
        format!(
            "ok       pty      opened a pseudo-terminal\n{NO_SCRIPTS_YET}ok       daemon   running, listening on {}\n",
            socket.display()
        )
    );
}

#[test]
fn doctor_checks_the_shell_integration_scripts_and_fix_brings_them_up_to_date() {
    let at = AgentTerminal::new();
    let scripts = at.home().join("shell");
    let script = scripts.join("bash/agent-terminal.bash");
    at.run_with(&["shell", "--shell", "bash", "--size", "40x5"], "", &[("HOME", &at.home().to_string_lossy())])
        .success();
    let shipped = std::fs::read_to_string(&script).expect("shell writes the bash integration script");
    let line = |run: &Run| run.stdout().lines().nth(1).map(str::to_string);
    let up_to_date = format!(
        "ok       shell    zsh (your SHELL) has shell integration; the scripts in {} are up to date",
        scripts.display()
    );
    assert_eq!(line(&doctor(&at, &["doctor"]).success()), Some(up_to_date.clone()));

    std::fs::write(&script, "an older version\n").expect("age the script");
    std::fs::create_dir_all(scripts.join("tcsh")).expect("leave a folder no version writes");
    std::fs::write(scripts.join("tcsh/agent-terminal.tcsh"), "stale\n").expect("leave a stale script");
    let stale = doctor(&at, &["doctor"]);
    assert_eq!(stale.code(), 1);
    assert_eq!(
        stale.stdout().lines().skip(1).take(2).collect::<Vec<_>>(),
        [
            format!(
                "problem  shell    the shell integration scripts in {} are not this version's (1 changed, 1 left over)",
                scripts.display()
            )
            .as_str(),
            "                  fix: agent-terminal doctor --fix brings them up to date",
        ]
    );
    assert_eq!(std::fs::read_to_string(&script).unwrap(), "an older version\n", "doctor without --fix changes nothing");

    let fixed = doctor(&at, &["doctor", "--fix"]).success();
    assert_eq!(
        line(&fixed),
        Some(format!(
            "fixed    shell    brought the shell integration scripts in {} up to date (rewrote 1, removed 1)",
            scripts.display()
        ))
    );
    assert_eq!(std::fs::read_to_string(&script).unwrap(), shipped);
    assert!(!scripts.join("tcsh").exists());
    assert_eq!(line(&doctor(&at, &["doctor"]).success()), Some(up_to_date));
    at.run(&["exec", "true"]).success();
}

#[test]
fn doctor_says_when_your_shell_has_no_integration() {
    let at = AgentTerminal::new();
    for (env, message) in [
        (Some("/bin/tcsh"), "SHELL is /bin/tcsh, which has no shell integration (zsh, bash and fish have it)"),
        (None, "SHELL is not set, so `agent-terminal shell` does not know which shell to open"),
    ] {
        let mut command = at.command(&["doctor"]);
        match env {
            Some(shell) => command.env("SHELL", shell),
            None => command.env_remove("SHELL"),
        };
        let run = Run(command.output().expect("run doctor"));
        assert_eq!(run.code(), 1);
        assert_eq!(
            run.stdout().lines().skip(1).take(2).collect::<Vec<_>>(),
            [
                format!("problem  shell    {message}").as_str(),
                "                  fix: open a shell with agent-terminal shell --shell zsh (or bash, fish)",
            ]
        );
    }
}

#[test]
fn doctor_reports_a_stale_socket_and_fix_removes_it() {
    let at = AgentTerminal::new();
    let socket = at.home().join("daemon.sock");
    drop(std::os::unix::net::UnixListener::bind(&socket).expect("leave a socket nobody listens on"));

    let run = doctor(&at, &["doctor"]);
    assert_eq!(run.code(), 1);
    assert_eq!(
        run.stdout(),
        format!(
            "ok       pty      opened a pseudo-terminal\n{NO_SCRIPTS_YET}problem  daemon   stale socket {}: no daemon is listening on it\n                  fix: agent-terminal doctor --fix removes it\n",
            socket.display()
        )
    );
    assert!(socket.exists(), "doctor without --fix must not change anything");

    let run = doctor(&at, &["doctor", "--fix"]).success();
    assert_eq!(
        run.stdout(),
        format!(
            "ok       pty      opened a pseudo-terminal\n{NO_SCRIPTS_YET}fixed    daemon   removed the stale socket {}\n",
            socket.display()
        )
    );
    assert!(!socket.exists());
    assert_eq!(
        doctor(&at, &["doctor"]).success().stdout().lines().nth(2),
        Some("ok       daemon   not running; the next command starts it")
    );
    at.run(&["open", "--size", "20x2", "cat"]).success();
}

#[test]
fn fixing_while_first_commands_start_the_daemon_leaves_one_working_daemon() {
    let at = AgentTerminal::new();
    drop(std::os::unix::net::UnixListener::bind(at.home().join("daemon.sock")).expect("leave a stale socket"));
    std::thread::scope(|scope| {
        let fixes: Vec<_> = (0..3).map(|_| scope.spawn(|| doctor(&at, &["doctor", "--fix"]).code())).collect();
        let opens: Vec<_> = (0..3).map(|_| scope.spawn(|| at.run(&["open", "--size", "20x2", "cat"]).code())).collect();
        let codes: Vec<i32> = fixes.into_iter().chain(opens).map(|call| call.join().unwrap()).collect();
        assert_eq!(codes, vec![0; 6]);
    });
    let closed = at.run(&["close", "--all"]).success().stdout();
    assert_eq!(closed.lines().count(), 3, "every open must land in the same daemon:\n{closed}");
}

#[test]
fn doctor_says_how_to_fix_a_socket_path_the_system_cannot_use() {
    let at = AgentTerminal::new();
    let home = at.home().join("x".repeat(120));
    let socket = home.join("daemon.sock");
    let run = at.run_with(&["doctor"], "", &[("AGENT_TERMINAL_HOME", &home.to_string_lossy()), ("SHELL", "/bin/zsh")]);
    assert_eq!(run.code(), 1);
    let stdout = run.stdout();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 4, "{stdout}");
    assert!(
        lines[2].starts_with(&format!("problem  daemon   cannot reach the daemon socket {}: ", socket.display())),
        "{stdout}"
    );
    assert_eq!(lines[3], "                  fix: set AGENT_TERMINAL_HOME to a short folder you own");
}

#[test]
fn doctor_json_lists_every_check() {
    let at = AgentTerminal::new();
    let healthy = doctor(&at, &["--json", "doctor"]).success().json();
    assert_eq!(
        healthy,
        serde_json::json!({
            "ok": true,
            "checks": [
                { "name": "pty", "status": "ok", "message": "opened a pseudo-terminal" },
                {
                    "name": "shell",
                    "status": "ok",
                    "message": "zsh (your SHELL) has shell integration; `agent-terminal shell` writes its scripts when it first runs"
                },
                { "name": "daemon", "status": "ok", "message": "not running; the next command starts it" }
            ]
        })
    );

    let socket = at.home().join("daemon.sock");
    drop(std::os::unix::net::UnixListener::bind(&socket).expect("leave a socket nobody listens on"));
    let run = doctor(&at, &["doctor", "--json"]);
    assert_eq!(run.code(), 1);
    assert_eq!(
        run.json()["checks"][2],
        serde_json::json!({
            "name": "daemon",
            "status": "problem",
            "message": format!("stale socket {}: no daemon is listening on it", socket.display()),
            "fix": "agent-terminal doctor --fix removes it"
        })
    );
    assert_eq!(run.json()["ok"], false);
}
