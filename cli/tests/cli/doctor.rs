use crate::support::AgentTerminal;

#[test]
fn doctor_checks_the_pty_the_fonts_and_the_daemon_without_starting_it() {
    let at = AgentTerminal::new();
    let socket = at.home().join("daemon.sock");
    let run = at.run(&["doctor"]).success();
    assert_eq!(
        run.stdout(),
        "ok       pty      opened a pseudo-terminal\nok       fonts    JetBrains Mono, Symbols Nerd Font Mono and Noto Emoji are built in\nok       daemon   not running; the next command starts it\n"
    );
    assert!(!socket.exists(), "doctor must not start the daemon");

    at.run(&["open", "--size", "20x2", "cat"]).success();
    let run = at.run(&["doctor"]).success();
    assert_eq!(
        run.stdout(),
        format!(
            "ok       pty      opened a pseudo-terminal\nok       fonts    JetBrains Mono, Symbols Nerd Font Mono and Noto Emoji are built in\nok       daemon   running, listening on {}\n",
            socket.display()
        )
    );
}

#[test]
fn doctor_reports_a_stale_socket_and_fix_removes_it() {
    let at = AgentTerminal::new();
    let socket = at.home().join("daemon.sock");
    drop(std::os::unix::net::UnixListener::bind(&socket).expect("leave a socket nobody listens on"));

    let run = at.run(&["doctor"]);
    assert_eq!(run.code(), 1);
    assert_eq!(
        run.stdout(),
        format!(
            "ok       pty      opened a pseudo-terminal\nok       fonts    JetBrains Mono, Symbols Nerd Font Mono and Noto Emoji are built in\nproblem  daemon   stale socket {}: no daemon is listening on it\n                  fix: agent-terminal doctor --fix removes it\n",
            socket.display()
        )
    );
    assert!(socket.exists(), "doctor without --fix must not change anything");

    let run = at.run(&["doctor", "--fix"]).success();
    assert_eq!(
        run.stdout(),
        format!(
            "ok       pty      opened a pseudo-terminal\nok       fonts    JetBrains Mono, Symbols Nerd Font Mono and Noto Emoji are built in\nfixed    daemon   removed the stale socket {}\n",
            socket.display()
        )
    );
    assert!(!socket.exists());
    assert_eq!(
        at.run(&["doctor"]).success().stdout().lines().nth(2),
        Some("ok       daemon   not running; the next command starts it")
    );
    at.run(&["open", "--size", "20x2", "cat"]).success();
}

#[test]
fn fixing_while_first_commands_start_the_daemon_leaves_one_working_daemon() {
    let at = AgentTerminal::new();
    drop(std::os::unix::net::UnixListener::bind(at.home().join("daemon.sock")).expect("leave a stale socket"));
    std::thread::scope(|scope| {
        let fixes: Vec<_> = (0..3).map(|_| scope.spawn(|| at.run(&["doctor", "--fix"]).code())).collect();
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
    let run = at.run_with(&["doctor"], "", &[("AGENT_TERMINAL_HOME", &home.to_string_lossy())]);
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
    let healthy = at.run(&["--json", "doctor"]).success().json();
    assert_eq!(
        healthy,
        serde_json::json!({
            "ok": true,
            "checks": [
                { "name": "pty", "status": "ok", "message": "opened a pseudo-terminal" },
                {
                    "name": "fonts",
                    "status": "ok",
                    "message": "JetBrains Mono, Symbols Nerd Font Mono and Noto Emoji are built in"
                },
                { "name": "daemon", "status": "ok", "message": "not running; the next command starts it" }
            ]
        })
    );

    let socket = at.home().join("daemon.sock");
    drop(std::os::unix::net::UnixListener::bind(&socket).expect("leave a socket nobody listens on"));
    let run = at.run(&["doctor", "--json"]);
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
