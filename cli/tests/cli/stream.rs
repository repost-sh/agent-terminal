use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::support::{AgentTerminal, DashboardSocket, dashboard, fixture};

const STREAM: &str = "/api/session/default/t1/stream";
const COUNTED: Duration = Duration::from_secs(5);

fn open_redraw(at: &AgentTerminal) {
    at.run(&["open", "--size", "120x40", "--wait", "ready", &fixture("redraw"), "--hz", "120", "--partial"]).success();
}

fn number(message: &Value) -> u64 {
    assert_eq!(message["type"], "frame", "{}", message["type"]);
    message["frame"].as_u64().expect("a frame number")
}

fn shown(message: &Value) -> Value {
    json!({ "type": "shown", "frame": number(message) })
}

fn current_frame(at: &AgentTerminal) -> u64 {
    at.run(&["get", "frame"]).success().stdout().trim().parse().expect("the tab's frame")
}

fn watch_and_acknowledge(mut socket: DashboardSocket) -> (u32, u64) {
    let started = Instant::now();
    let mut counted = 0;
    let mut last_frame = 0;
    while let Some(message) = socket.try_next(Duration::from_secs(1)) {
        socket.send(&shown(&message));
        if started.elapsed() <= COUNTED {
            counted += 1;
        }
        last_frame = number(&message);
    }
    (counted, last_frame)
}

#[test]
fn a_fast_tab_is_delivered_at_most_thirty_times_a_second_to_each_viewer_and_ends_on_its_latest_frame() {
    let at = AgentTerminal::new();
    open_redraw(&at);
    let port = dashboard(&at);
    let viewers: Vec<_> = (0..2)
        .map(|_| DashboardSocket::open(port, STREAM))
        .map(|socket| thread::spawn(move || watch_and_acknowledge(socket)))
        .collect();

    thread::sleep(COUNTED);
    at.run(&["signal", "KILL"]).success();
    let delivered: Vec<(u32, u64)> = viewers.into_iter().map(|viewer| viewer.join().expect("a viewer")).collect();
    let latest = current_frame(&at);

    for (counted, last_frame) in delivered {
        assert!((100..=175).contains(&counted), "a viewer received {counted} frames in {COUNTED:?}, not 100 to 175");
        assert_eq!(last_frame, latest, "a viewer ended on frame {last_frame}, the tab on {latest}");
    }
}

#[test]
fn a_viewer_that_stops_acknowledging_receives_two_frames_and_then_one_per_acknowledgement() {
    let at = AgentTerminal::new();
    open_redraw(&at);
    let mut socket = DashboardSocket::open(dashboard(&at), STREAM);

    let mut received = Vec::new();
    while received.len() <= 2 {
        match socket.try_next(Duration::from_millis(500)) {
            Some(message) => received.push(number(&message)),
            None => break,
        }
    }
    assert_eq!(received.len(), 2, "frames before the viewer acknowledged any: {received:?}");

    socket.send(&json!({ "type": "shown", "frame": received[0] }));
    let after = socket.try_next(Duration::from_secs(1)).expect("a frame after the acknowledgement");
    assert!(number(&after) > received[1], "frame {} after frame {}", number(&after), received[1]);
    assert_eq!(socket.try_next(Duration::from_millis(500)).map(|message| number(&message)), None);
}

#[test]
fn a_single_change_on_a_quiet_terminal_is_one_message_carrying_the_current_frame() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x4", "--wait", "ready", "--", "sh", "-c", "printf \"ready\\n\"; cat"]).success();
    let mut socket = DashboardSocket::open(dashboard(&at), STREAM);
    let first = socket.next();
    socket.send(&shown(&first));

    at.run(&["type", "a"]).success();
    let typed = socket.try_next(Duration::from_secs(2)).expect("a frame for the typed key");
    assert!(typed["paint"].as_str().expect("a paint").contains('a'), "{typed}");
    assert_eq!(number(&typed), current_frame(&at));
    assert_eq!(socket.try_next(Duration::from_millis(500)), None);
}
