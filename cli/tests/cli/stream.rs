use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::support::{AgentTerminal, DashboardSocket, Redraw, dashboard, frame_number, open_redraw};

const STREAM: &str = "/api/session/default/t1/stream";
const COUNTED: Duration = Duration::from_secs(5);

fn watch_and_acknowledge(mut socket: DashboardSocket, window_closed: &mpsc::Sender<()>) -> (u32, u64) {
    let started = Instant::now();
    let mut counted = 0;
    let mut last_frame = 0;
    let mut told = false;
    while let Some(message) = socket.try_next(Duration::from_secs(1)) {
        last_frame = socket.acknowledge(&message);
        let in_window = started.elapsed() <= COUNTED;
        counted += u32::from(in_window);
        if !in_window && !told {
            told = true;
            window_closed.send(()).expect("the test waits for the counted window to close");
        }
    }
    (counted, last_frame)
}

#[test]
fn a_fast_tab_is_delivered_at_most_thirty_times_a_second_to_each_viewer_and_ends_on_its_latest_frame() {
    let at = AgentTerminal::new();
    open_redraw(&at, &Redraw { session: "default", hz: "120", repaint: "--partial" });
    let port = dashboard(&at);
    let (window_closed, closed) = mpsc::channel();
    let viewers: Vec<_> = (0..2)
        .map(|_| (DashboardSocket::open(port, STREAM), window_closed.clone()))
        .map(|(socket, window_closed)| thread::spawn(move || watch_and_acknowledge(socket, &window_closed)))
        .collect();
    drop(window_closed);

    for _ in &viewers {
        closed.recv().expect("each viewer receives frames past the counted window");
    }
    at.run(&["signal", "KILL"]).success();
    let delivered: Vec<(u32, u64)> = viewers.into_iter().map(|viewer| viewer.join().expect("a viewer")).collect();
    let latest = at.frame("default");

    for (counted, last_frame) in delivered {
        assert!((100..=175).contains(&counted), "a viewer received {counted} frames in {COUNTED:?}, not 100 to 175");
        assert_eq!(last_frame, latest, "a viewer ended on frame {last_frame}, the tab on {latest}");
    }
}

#[test]
fn a_viewer_that_stops_acknowledging_receives_two_frames_and_then_one_per_acknowledgement() {
    let at = AgentTerminal::new();
    open_redraw(&at, &Redraw { session: "default", hz: "120", repaint: "--partial" });
    let mut socket = DashboardSocket::open(dashboard(&at), STREAM);

    let mut received = Vec::new();
    while received.len() <= 2 {
        match socket.try_next(Duration::from_millis(500)) {
            Some(message) => received.push(message),
            None => break,
        }
    }
    assert_eq!(received.len(), 2, "frames before the viewer acknowledged any: {received:?}");

    socket.acknowledge(&received[0]);
    let after = frame_number(&socket.try_next(Duration::from_secs(1)).expect("a frame after the acknowledgement"));
    let second = frame_number(&received[1]);
    assert!(after > second, "frame {after} after frame {second}");
    assert_eq!(socket.try_next(Duration::from_millis(500)).map(|message| frame_number(&message)), None);
}

#[test]
fn a_single_change_on_a_quiet_terminal_is_one_message_carrying_the_current_frame() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x4", "--wait", "ready", "--", "sh", "-c", "printf \"ready\\n\"; cat"]).success();
    let mut socket = DashboardSocket::open(dashboard(&at), STREAM);
    socket.acknowledge_next();

    at.run(&["type", "a"]).success();
    let typed = socket.try_next(Duration::from_secs(2)).expect("a frame for the typed key");
    assert!(typed["paint"].as_str().expect("a paint").contains('a'), "{typed}");
    assert_eq!(frame_number(&typed), at.frame("default"));
    assert_eq!(socket.try_next(Duration::from_millis(500)), None);
}
