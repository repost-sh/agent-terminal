use crate::skills::{put_stub, shipped, stub_in, stub_of_version};
use crate::support::{AgentTerminal, Run, VERSION};

const FONTS: &str = "ok       fonts    JetBrains Mono, Symbols Nerd Font Mono and Noto Emoji are built in\n";

const NO_SCRIPTS_YET: &str = "ok       shell    zsh (your SHELL) has shell integration; `agent-terminal shell` writes its scripts when it first runs\n";

fn user(at: &AgentTerminal) -> std::path::PathBuf {
    at.home().join("user")
}

fn stub(at: &AgentTerminal) -> std::path::PathBuf {
    stub_in(&user(at))
}

fn no_skill(at: &AgentTerminal) -> String {
    format!(
        "ok       skill    the agent-terminal skill is not installed at {}; `agent-terminal install --skills` installs it\n",
        stub(at).display()
    )
}

fn doctor(at: &AgentTerminal, args: &[&str]) -> Run {
    at.run_with(args, "", &[("SHELL", "/bin/zsh"), ("HOME", &user(at).to_string_lossy())])
}

#[test]
fn doctor_checks_the_pty_the_fonts_and_the_daemon_without_starting_it() {
    let at = AgentTerminal::new();
    let socket = at.home().join("daemon.sock");
    let run = doctor(&at, &["doctor"]).success();
    assert_eq!(
        run.stdout(),
        format!(
            "ok       pty      opened a pseudo-terminal\n{FONTS}{NO_SCRIPTS_YET}ok       daemon   not running; the next command starts it\n{}",
            no_skill(&at)
        )
    );
    assert!(!socket.exists(), "doctor must not start the daemon");

    at.run(&["open", "--size", "20x2", "cat"]).success();
    let run = doctor(&at, &["doctor"]).success();
    assert_eq!(
        run.stdout(),
        format!(
            "ok       pty      opened a pseudo-terminal\n{FONTS}{NO_SCRIPTS_YET}ok       daemon   running agent-terminal {VERSION}, listening on {}\n{}",
            socket.display(),
            no_skill(&at)
        )
    );
}

#[test]
fn doctor_flags_a_daemon_of_another_version_and_says_how_to_replace_it() {
    let at = AgentTerminal::new();
    let socket = at.home().join("daemon.sock");
    at.run(&["open", "--size", "20x2", "cat"]).success();
    std::fs::write(at.home().join("daemon.version"), "0.0.1").expect("make the daemon report another version");

    the_text_report_flags_the_other_version(&at, &socket);
    the_json_report_flags_the_other_version(&at, &socket);
    assert_eq!(
        std::fs::read_to_string(at.home().join("daemon.version")).unwrap(),
        "0.0.1",
        "doctor leaves the daemon running"
    );
}

fn the_text_report_flags_the_other_version(at: &AgentTerminal, socket: &std::path::Path) {
    let run = doctor(at, &["doctor"]);
    assert_eq!(run.code(), 1);
    assert_eq!(
        run.stdout().lines().skip(3).take(2).collect::<Vec<_>>(),
        [
            format!(
                "problem  daemon   running agent-terminal 0.0.1, not {VERSION} like this command, listening on {}",
                socket.display()
            )
            .as_str(),
            &format!(
                "                  fix: `agent-terminal daemon stop` ends it and its terminals, and the next command starts a {VERSION} daemon"
            ),
        ]
    );
}

fn the_json_report_flags_the_other_version(at: &AgentTerminal, socket: &std::path::Path) {
    let run = doctor(at, &["--json", "doctor"]);
    assert_eq!(run.code(), 1);
    assert_eq!(run.json()["ok"], false);
    assert_eq!(
        run.json()["checks"][3],
        serde_json::json!({
            "name": "daemon",
            "status": "problem",
            "message": format!(
                "running agent-terminal 0.0.1, not {VERSION} like this command, listening on {}",
                socket.display()
            ),
            "fix": format!(
                "`agent-terminal daemon stop` ends it and its terminals, and the next command starts a {VERSION} daemon"
            )
        })
    );
}

fn shell_line(run: &Run) -> Option<String> {
    run.stdout().lines().nth(2).map(str::to_string)
}

#[test]
fn doctor_checks_the_shell_integration_scripts_and_fix_brings_them_up_to_date() {
    let at = AgentTerminal::new();
    let scripts = at.home().join("shell");
    let script = scripts.join("bash/agent-terminal.bash");
    at.run_with(&["shell", "--shell", "bash", "--size", "40x5"], "", &[("HOME", &at.home().to_string_lossy())])
        .success();
    let shipped = std::fs::read_to_string(&script).expect("shell writes the bash integration script");
    let up_to_date = format!(
        "ok       shell    zsh (your SHELL) has shell integration; the scripts in {} are up to date",
        scripts.display()
    );
    assert_eq!(shell_line(&doctor(&at, &["doctor"]).success()), Some(up_to_date.clone()));

    changed_and_left_over_scripts_are_a_problem(&at, &scripts, &script);
    fix_rewrites_the_changed_script_and_removes_the_left_over(&at, &scripts, &script, &shipped);
    assert_eq!(shell_line(&doctor(&at, &["doctor"]).success()), Some(up_to_date));
    at.run(&["exec", "true"]).success();
}

fn changed_and_left_over_scripts_are_a_problem(
    at: &AgentTerminal,
    scripts: &std::path::Path,
    script: &std::path::Path,
) {
    std::fs::write(script, "an older version\n").expect("age the script");
    std::fs::create_dir_all(scripts.join("tcsh")).expect("leave a folder no version writes");
    std::fs::write(scripts.join("tcsh/agent-terminal.tcsh"), "stale\n").expect("leave a stale script");
    let stale = doctor(at, &["doctor"]);
    assert_eq!(stale.code(), 1);
    assert_eq!(
        stale.stdout().lines().skip(2).take(2).collect::<Vec<_>>(),
        [
            format!(
                "problem  shell    the shell integration scripts in {} are not this version's (1 changed, 1 left over)",
                scripts.display()
            )
            .as_str(),
            "                  fix: agent-terminal doctor --fix brings them up to date",
        ]
    );
    assert_eq!(std::fs::read_to_string(script).unwrap(), "an older version\n", "doctor without --fix changes nothing");
}

fn fix_rewrites_the_changed_script_and_removes_the_left_over(
    at: &AgentTerminal,
    scripts: &std::path::Path,
    script: &std::path::Path,
    shipped: &str,
) {
    let fixed = doctor(at, &["doctor", "--fix"]).success();
    assert_eq!(
        shell_line(&fixed),
        Some(format!(
            "fixed    shell    brought the shell integration scripts in {} up to date (rewrote 1, removed 1)",
            scripts.display()
        ))
    );
    assert_eq!(std::fs::read_to_string(script).unwrap(), shipped);
    assert!(!scripts.join("tcsh").exists());
}

#[test]
fn doctor_says_when_your_shell_has_no_integration() {
    let at = AgentTerminal::new();
    for (env, message) in [
        (Some("/bin/tcsh"), "SHELL is /bin/tcsh, which has no shell integration (zsh, bash and fish have it)"),
        (None, "SHELL is not set, so `agent-terminal shell` does not know which shell to open"),
    ] {
        let mut command = at.command(&["doctor"]);
        command.env("HOME", user(&at));
        match env {
            Some(shell) => command.env("SHELL", shell),
            None => command.env_remove("SHELL"),
        };
        let run = Run(command.output().expect("run doctor"));
        assert_eq!(run.code(), 1);
        assert_eq!(
            run.stdout().lines().skip(2).take(2).collect::<Vec<_>>(),
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

    the_stale_socket_is_reported_and_kept(&at, &socket);
    fix_removes_the_stale_socket(&at, &socket);
    at.run(&["open", "--size", "20x2", "cat"]).success();
}

fn the_stale_socket_is_reported_and_kept(at: &AgentTerminal, socket: &std::path::Path) {
    let run = doctor(at, &["doctor"]);
    assert_eq!(run.code(), 1);
    assert_eq!(
        run.stdout(),
        format!(
            "ok       pty      opened a pseudo-terminal\n{FONTS}{NO_SCRIPTS_YET}problem  daemon   stale socket {}: no daemon is listening on it\n                  fix: agent-terminal doctor --fix removes it\n{}",
            socket.display(),
            no_skill(at)
        )
    );
    assert!(socket.exists(), "doctor without --fix must not change anything");
}

fn fix_removes_the_stale_socket(at: &AgentTerminal, socket: &std::path::Path) {
    let run = doctor(at, &["doctor", "--fix"]).success();
    assert_eq!(
        run.stdout(),
        format!(
            "ok       pty      opened a pseudo-terminal\n{FONTS}{NO_SCRIPTS_YET}fixed    daemon   removed the stale socket {}\n{}",
            socket.display(),
            no_skill(at)
        )
    );
    assert!(!socket.exists());
    assert_eq!(
        doctor(at, &["doctor"]).success().stdout().lines().nth(3),
        Some("ok       daemon   not running; the next command starts it")
    );
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
    let run = at.run_with(
        &["doctor"],
        "",
        &[
            ("AGENT_TERMINAL_HOME", &home.to_string_lossy()),
            ("SHELL", "/bin/zsh"),
            ("HOME", &user(&at).to_string_lossy()),
        ],
    );
    assert_eq!(run.code(), 1);
    let stdout = run.stdout();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 6, "{stdout}");
    assert!(
        lines[3].starts_with(&format!("problem  daemon   cannot reach the daemon socket {}: ", socket.display())),
        "{stdout}"
    );
    assert_eq!(lines[4], "                  fix: set AGENT_TERMINAL_HOME to a short folder you own");
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
                    "name": "fonts",
                    "status": "ok",
                    "message": "JetBrains Mono, Symbols Nerd Font Mono and Noto Emoji are built in"
                },
                {
                    "name": "shell",
                    "status": "ok",
                    "message": "zsh (your SHELL) has shell integration; `agent-terminal shell` writes its scripts when it first runs"
                },
                { "name": "daemon", "status": "ok", "message": "not running; the next command starts it" },
                {
                    "name": "skill",
                    "status": "ok",
                    "message": format!(
                        "the agent-terminal skill is not installed at {}; `agent-terminal install --skills` installs it",
                        stub(&at).display()
                    )
                }
            ]
        })
    );
    a_stale_socket_makes_the_json_report_not_ok(&at);
}

fn a_stale_socket_makes_the_json_report_not_ok(at: &AgentTerminal) {
    let socket = at.home().join("daemon.sock");
    drop(std::os::unix::net::UnixListener::bind(&socket).expect("leave a socket nobody listens on"));
    let run = doctor(at, &["doctor", "--json"]);
    assert_eq!(run.code(), 1);
    assert_eq!(
        run.json()["checks"][3],
        serde_json::json!({
            "name": "daemon",
            "status": "problem",
            "message": format!("stale socket {}: no daemon is listening on it", socket.display()),
            "fix": "agent-terminal doctor --fix removes it"
        })
    );
    assert_eq!(run.json()["ok"], false);
}

fn skill_lines(run: &Run) -> Vec<String> {
    run.stdout().lines().skip(4).map(str::to_string).collect()
}

#[test]
fn doctor_says_the_installed_skill_is_this_version() {
    let at = AgentTerminal::new();
    at.run_with_env(&["install", "--skills"], &[("HOME", &user(&at))]).success();
    let message = format!("the agent-terminal skill at {} is this version ({VERSION})", stub(&at).display());

    let run = doctor(&at, &["doctor"]).success();
    assert_eq!(skill_lines(&run), [format!("ok       skill    {message}")]);

    let json = doctor(&at, &["doctor", "--json"]).success().json();
    assert_eq!(json["checks"][4], serde_json::json!({ "name": "skill", "status": "ok", "message": message }));
    assert_eq!(json["ok"], true);
}

fn installed(at: &AgentTerminal) -> String {
    std::fs::read_to_string(stub(at)).unwrap()
}

struct SkillProblem {
    problem: String,
    fix: &'static str,
}

fn a_stale_skill_is_a_problem_and_fix_rewrites_it(stale: &str, found: &str) {
    let at = AgentTerminal::new();
    let current = shipped("skills/agent-terminal/SKILL.md");
    let report = SkillProblem {
        problem: format!("the agent-terminal skill at {} {found}", stub(&at).display()),
        fix: "agent-terminal doctor --fix rewrites it",
    };
    let fixed = format!("rewrote the agent-terminal skill at {} as this version ({VERSION})", stub(&at).display());
    put_stub(&user(&at), stale);

    the_stale_skill_is_a_problem_in_text(&at, &report);
    the_stale_skill_is_a_problem_in_json(&at, &report);
    assert_eq!(installed(&at), stale, "doctor without --fix changes nothing");
    fix_rewrites_the_stale_skill(&at, &fixed, &current);
    put_stub(&user(&at), stale);
    fix_in_json_rewrites_the_stale_skill(&at, &fixed, &current);
    assert_eq!(
        skill_lines(&doctor(&at, &["doctor"]).success()),
        [format!("ok       skill    the agent-terminal skill at {} is this version ({VERSION})", stub(&at).display())]
    );
}

fn the_stale_skill_is_a_problem_in_text(at: &AgentTerminal, report: &SkillProblem) {
    let run = doctor(at, &["doctor"]);
    assert_eq!(run.code(), 1);
    assert_eq!(
        skill_lines(&run),
        [format!("problem  skill    {}", report.problem), format!("                  fix: {}", report.fix)]
    );
}

fn the_stale_skill_is_a_problem_in_json(at: &AgentTerminal, report: &SkillProblem) {
    let run = doctor(at, &["doctor", "--json"]);
    assert_eq!(run.code(), 1);
    assert_eq!(
        run.json()["checks"][4],
        serde_json::json!({ "name": "skill", "status": "problem", "message": report.problem, "fix": report.fix })
    );
    assert_eq!(run.json()["ok"], false);
}

fn fix_rewrites_the_stale_skill(at: &AgentTerminal, fixed: &str, current: &str) {
    let run = doctor(at, &["doctor", "--fix"]).success();
    assert_eq!(skill_lines(&run), [format!("fixed    skill    {fixed}")]);
    assert_eq!(installed(at), current);
}

fn fix_in_json_rewrites_the_stale_skill(at: &AgentTerminal, fixed: &str, current: &str) {
    let json = doctor(at, &["doctor", "--fix", "--json"]).success().json();
    assert_eq!(json["checks"][4], serde_json::json!({ "name": "skill", "status": "fixed", "message": fixed }));
    assert_eq!(json["ok"], true);
    assert_eq!(installed(at), current);
}

#[test]
fn doctor_reports_an_older_skill_and_fix_rewrites_it() {
    a_stale_skill_is_a_problem_and_fix_rewrites_it(
        &stub_of_version("0.0.1"),
        &format!("is 0.0.1, older than this version ({VERSION})"),
    );
}

#[test]
fn doctor_reports_an_unversioned_skill_and_fix_rewrites_it() {
    a_stale_skill_is_a_problem_and_fix_rewrites_it(
        "# my notes\n",
        &format!("has no version, so it may not be this version ({VERSION})"),
    );
}

#[test]
fn doctor_reports_a_newer_skill_and_fix_leaves_it() {
    let at = AgentTerminal::new();
    let newer = stub_of_version("999.0.0");
    put_stub(&user(&at), &newer);
    let report = SkillProblem {
        problem: format!(
            "the agent-terminal skill at {} is 999.0.0, newer than this version ({VERSION})",
            stub(&at).display()
        ),
        fix: "agent-terminal upgrade brings agent-terminal up to it (doctor --fix does not downgrade a skill)",
    };

    for args in [&["doctor"][..], &["doctor", "--fix"]] {
        a_newer_skill_is_a_problem_in_text(&at, args, &report);
        a_newer_skill_is_a_problem_in_json_and_left_alone(&at, args, &report, &newer);
    }
}

fn a_newer_skill_is_a_problem_in_text(at: &AgentTerminal, args: &[&str], report: &SkillProblem) {
    let run = doctor(at, args);
    assert_eq!(run.code(), 1, "{args:?}");
    assert_eq!(
        skill_lines(&run),
        [format!("problem  skill    {}", report.problem), format!("                  fix: {}", report.fix)]
    );
}

fn a_newer_skill_is_a_problem_in_json_and_left_alone(
    at: &AgentTerminal,
    args: &[&str],
    report: &SkillProblem,
    newer: &str,
) {
    let run = doctor(at, &[args, &["--json"]].concat());
    assert_eq!(run.code(), 1, "{args:?}");
    assert_eq!(
        run.json()["checks"][4],
        serde_json::json!({ "name": "skill", "status": "problem", "message": report.problem, "fix": report.fix })
    );
    assert_eq!(run.json()["ok"], false);
    assert_eq!(installed(at), newer, "{args:?} must leave a newer skill alone");
}

#[test]
fn doctor_says_it_cannot_find_the_skill_without_home() {
    let at = AgentTerminal::new();
    let run = Run(at.command(&["doctor"]).env("SHELL", "/bin/zsh").env_remove("HOME").output().expect("run doctor"));
    assert_eq!(run.code(), 1);
    assert_eq!(
        skill_lines(&run),
        [
            "problem  skill    HOME is not set; it locates the agent skills folder ~/.claude/skills",
            "                  fix: check that HOME is set and that you can read and write ~/.claude/skills/agent-terminal/SKILL.md",
        ]
    );
}
