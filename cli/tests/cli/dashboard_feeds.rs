use serde_json::Value;

use crate::support::{AgentTerminal, DashboardSocket, dashboard, fixture};

fn feed(port: u16, name: &str) -> DashboardSocket {
    DashboardSocket::open(port, &format!("/api/session/work/app/{name}"))
}

fn until_closed(socket: &mut DashboardSocket) -> Vec<Value> {
    let mut updates = vec![socket.next()];
    while updates.last().is_some_and(|update| update["type"] != "closed") {
        updates.push(socket.next());
    }
    updates
}

#[test]
fn each_tab_feed_says_closed_when_its_tab_closes() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "20x3", "--wait", "line 3", &fixture("line-printer"), "--hold"])
        .success();
    let port = dashboard(&at);
    let mut feeds: Vec<DashboardSocket> =
        ["activity", "output", "info"].into_iter().map(|name| feed(port, name)).collect();
    let first: Vec<Value> = feeds.iter_mut().map(|socket| socket.next()["type"].clone()).collect();
    assert_eq!(first, ["entries", "output", "info"]);
    at.run(&["close"]).success();
    let ends: Vec<Value> =
        feeds.iter_mut().map(|socket| until_closed(socket).last().cloned().unwrap_or_default()).collect();
    assert_eq!(ends, vec![serde_json::json!({ "type": "closed" }); 3]);
}

fn texts(update: &Value, stream: &str) -> String {
    update[stream].as_array().into_iter().flatten().filter_map(|segment| segment["text"].as_str()).collect()
}

#[test]
fn the_output_feed_decodes_text_split_across_reads_and_marks_bytes_that_are_not_utf8() {
    let at = AgentTerminal::new();
    let script = r"stty -echo; printf 'a\303'; read _; printf '\251\377b'; read _";
    at.run(&["open", "--name", "app", "--size", "20x3", "--wait", "a", "sh", "-c", script]).success();
    let mut output = feed(dashboard(&at), "output");
    let mut updates = vec![output.next()];
    at.run(&["press", "Enter"]).success();
    while !updates.iter().any(|update| texts(update, "raw").contains('b')) {
        updates.push(output.next());
    }
    let joined = |stream| updates.iter().map(|update| texts(update, stream)).collect::<String>();
    assert_eq!((joined("raw"), joined("styled")), ("aé\u{fffd}b".to_string(), "aé\u{fffd}b".to_string()));
}

#[test]
fn an_unreadable_activity_log_ends_the_activity_feed_and_says_why_in_the_daemon_log() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "20x3", "--wait", "line 3", &fixture("line-printer"), "--hold"])
        .success();
    std::fs::write(at.home().join("sessions/work/activity.jsonl"), "not json\n").expect("break the activity log");
    let mut activity = feed(dashboard(&at), "activity");
    assert!(activity.closed());
    assert!(
        at.daemon_log().contains("dashboard: reading the activity of work app failed: read "),
        "{}",
        at.daemon_log()
    );
}
