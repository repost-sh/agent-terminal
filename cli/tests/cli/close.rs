use crate::support::{AgentTerminal, fixture};

const SESSION_NOT_TYPED: &str = "error: close --all closes every tab of one session, so name that session on this command line: repeat it as `--session <name>` (AGENT_TERMINAL_SESSION alone is not enough); `close --all --bypass-session` closes every tab in every session\n";

fn two_sessions() -> AgentTerminal {
    let at = AgentTerminal::without_session();
    for session in ["a", "b"] {
        at.run(&["session", "new", session]).success();
        at.run(&["--session", session, "open", "--name", "app", &fixture("line-printer"), "--hold"]).success();
    }
    at
}

fn sessions_left(at: &AgentTerminal) -> Vec<String> {
    let listed = at.run(&["--json", "session", "list"]).success().json();
    let sessions = listed["sessions"].as_array().expect("the sessions");
    sessions.iter().map(|session| session["session"].as_str().expect("a session name").to_string()).collect()
}

#[test]
fn closing_the_last_tab_leaves_the_session_listed_with_no_tabs() {
    let at = two_sessions();
    assert_eq!(at.run(&["--session", "a", "close"]).success().stdout(), "closed a app\n");
    assert_eq!(at.run(&["session", "list"]).success().stdout().lines().next(), Some("a · no tabs"));
    assert_eq!(at.run(&["--session", "a", "tab"]).success().stdout(), "no tabs are open in session a\n");
}

#[test]
fn close_all_with_a_session_ends_it_so_its_name_is_free_again() {
    let at = two_sessions();
    assert_eq!(at.run(&["--session", "a", "close", "--all"]).success().stdout(), "closed a app\nended session a\n");
    let gone = at.run(&["--session", "a", "open", "--name", "app", "cat"]);
    assert_eq!(
        (gone.code(), gone.stderr()),
        (1, "error: no session a; create it with `agent-terminal session new a`\n".to_string())
    );
    assert_eq!(at.run(&["session", "new", "a"]).success().stdout(), "created session a\n");
}

#[test]
fn close_all_with_a_session_closes_that_sessions_tabs_and_leaves_the_others_running() {
    let at = two_sessions();
    assert_eq!(at.run(&["--session", "a", "close", "--all"]).success().stdout(), "closed a app\nended session a\n");
    assert_eq!(sessions_left(&at), ["b"]);
    assert_eq!(at.run(&["close", "--all", "--session", "b"]).success().stdout(), "closed b app\nended session b\n");
    assert_eq!(sessions_left(&at), Vec::<String>::new());
}

#[test]
fn close_all_refuses_a_session_named_only_by_the_environment_and_closes_nothing() {
    let at = two_sessions();
    let refused = at.run_with_session_env("a", &["close", "--all"]);
    assert_eq!((refused.code(), refused.stdout(), refused.stderr()), (1, String::new(), SESSION_NOT_TYPED.to_string()));
    let unnamed = at.run(&["close", "--all"]);
    assert_eq!((unnamed.code(), unnamed.stderr()), (1, SESSION_NOT_TYPED.to_string()));
    assert_eq!(sessions_left(&at), ["a", "b"]);
}

#[test]
fn close_all_bypass_session_closes_every_tab_in_every_session_even_with_a_session_in_the_environment() {
    let at = two_sessions();
    let closed = at.run_with_session_env("a", &["close", "--all", "--bypass-session"]).success();
    assert_eq!(closed.stdout(), "closed a app\nclosed b app\nended session a\nended session b\n");
    assert_eq!(sessions_left(&at), Vec::<String>::new());
}

#[test]
fn bypass_session_with_session_or_without_all_is_a_usage_error_and_closes_nothing() {
    let at = two_sessions();
    let both = at.run(&["--session", "a", "close", "--all", "--bypass-session"]);
    assert_eq!(
        (both.code(), both.stderr()),
        (
            1,
            "error: the argument '--bypass-session' cannot be used with '--session <NAME>': close --all --session <name> closes the tabs of that session, close --all --bypass-session closes every tab in every session\n".to_string()
        )
    );
    let alone = at.run(&["close", "--bypass-session"]);
    assert_eq!(alone.code(), 1);
    assert!(
        alone.stderr().contains("the following required arguments were not provided:\n  --all"),
        "{}",
        alone.stderr()
    );
    assert_eq!(sessions_left(&at), ["a", "b"]);
}
