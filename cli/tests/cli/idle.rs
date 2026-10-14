use std::io::Read;
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::support::{AgentTerminal, Run, fixture};

const BOUND: Duration = Duration::from_secs(10);

struct Held {
    fifo: String,
    gone: mpsc::Receiver<Instant>,
}

fn hold(at: &AgentTerminal) -> Held {
    let fifo = at.home().join("held");
    assert!(Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    let (released, gone) = mpsc::channel();
    let reading = fifo.clone();
    std::thread::spawn(move || {
        let mut rest = Vec::new();
        let _ = std::fs::File::open(&reading).and_then(|mut held| held.read_to_end(&mut rest));
        let _ = released.send(Instant::now());
    });
    Held { fifo: fifo.to_string_lossy().into_owned(), gone }
}

fn open_holding(at: &AgentTerminal, idle_timeout: &str, held: &Held) -> Run {
    at.run(&[
        "--idle-timeout",
        idle_timeout,
        "open",
        "--size",
        "20x2",
        "--wait",
        "holding",
        "sh",
        "-c",
        "exec 3>\"$0\"; echo holding; read _",
        &held.fifo,
    ])
}

#[test]
fn an_idle_daemon_stops_its_programs_and_the_next_call_starts_it_again() {
    let at = AgentTerminal::new();
    let held = hold(&at);
    let before_last_call = Instant::now();
    open_holding(&at, "400ms", &held).success();

    let gone_at = held.gone.recv_timeout(BOUND).expect("the idle daemon stops the program it was running");
    let idle = gone_at - before_last_call;
    assert!(idle >= Duration::from_millis(400), "stopped after {idle:?}");
    assert!(at.daemon_log().contains("no call for 400 ms, stopping\n"), "{}", at.daemon_log());

    let after = at.run(&["snapshot"]);
    assert_eq!(after.code(), 1);
    assert!(after.stderr().contains("no terminal is open in session default"), "{}", after.stderr());
    assert_eq!(at.run(&["session", "list"]).success().stdout(), "no sessions are open\n");
}

#[test]
fn a_call_that_lasts_longer_than_the_idle_timeout_is_not_cut_off() {
    let at = AgentTerminal::new();
    let opened = at
        .run(&[
            "--idle-timeout",
            "300",
            "open",
            "--size",
            "20x4",
            "--wait",
            "line 3",
            &fixture("line-printer"),
            "--lines",
            "3",
            "--delay-ms",
            "300",
            "--hold",
        ])
        .success();
    assert_eq!(opened.stdout().lines().nth(3), Some("3│line 3"));
}

#[test]
fn an_idle_timeout_of_0_keeps_the_daemon_running() {
    let at = AgentTerminal::new();
    let held = hold(&at);
    open_holding(&at, "0", &held).success();

    assert!(held.gone.recv_timeout(Duration::from_secs(1)).is_err(), "the daemon stopped although --idle-timeout is 0");
    assert_eq!(at.run(&["snapshot"]).success().stdout().lines().nth(1), Some("1│holding"));
}

#[test]
fn an_idle_timeout_in_an_unknown_unit_is_refused() {
    let at = AgentTerminal::new();
    let run = at.run(&["--idle-timeout", "soon", "session"]);
    assert_ne!(run.code(), 0);
    assert!(run.stderr().contains("expected a duration like 30s, 10m, 1h or raw ms (0 = never)"), "{}", run.stderr());
}
