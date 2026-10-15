use std::net::TcpStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use tungstenite::{Message, WebSocket};

use crate::support::{AgentTerminal, Cue, fixture, snapshot_rows};

struct Timeline {
    socket: WebSocket<TcpStream>,
}

struct Run {
    started_ms: u64,
    frames: Vec<Value>,
}

impl Timeline {
    fn open(port: u16, tab: &str) -> Self {
        let stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to the dashboard");
        stream.set_read_timeout(Some(Duration::from_secs(25))).expect("bound every read");
        let url = format!("ws://localhost:{port}/api/session/default/{tab}/timeline");
        let (socket, _) = tungstenite::client(url.as_str(), stream).expect("open the timeline");
        Self { socket }
    }

    fn next(&mut self) -> Value {
        match self.socket.read().expect("a timeline message") {
            Message::Text(text) => serde_json::from_str(&text).expect("a JSON message"),
            other => panic!("expected a text message, got {other:?}"),
        }
    }

    fn run_through(&mut self, last: u64) -> Run {
        let first = self.next();
        assert_eq!(first["type"], "frames", "{first}");
        assert_eq!(first["reset"], true, "a run starts with a reset: {first}");
        let started_ms = first["started_ms"].as_u64().expect("the run's start");
        let mut frames: Vec<Value> = first["frames"].as_array().expect("frames").clone();
        while frames.last().and_then(|frame| frame["frame"].as_u64()).unwrap_or(0) < last {
            let more = self.next();
            assert_eq!(more["type"], "frames", "{more}");
            assert_eq!(more["reset"], false, "{more}");
            frames.extend(more["frames"].as_array().expect("frames").iter().cloned());
        }
        Run { started_ms, frames }
    }
}

fn dashboard(at: &AgentTerminal) -> u16 {
    let started = at.run(&["--json", "dashboard", "start", "--port", "0"]).success().json();
    started["port"].as_u64().expect("the dashboard's port") as u16
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("a clock after 1970").as_millis() as u64
}

fn number(frame: &Value) -> u64 {
    frame["frame"].as_u64().expect("a frame number")
}

fn redraw(screen: &mut [String], paint: &str) {
    let mut row: Option<usize> = None;
    let mut chars = paint.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            let mut params = String::new();
            let action = loop {
                let next = chars.next().expect("a complete escape sequence");
                if next.is_ascii_alphabetic() {
                    break next;
                }
                params.push(next);
            };
            match action {
                'H' => row = params.split(';').next().and_then(|n| n.parse::<usize>().ok()).map(|n| n - 1),
                'K' => screen[row.expect("a row to clear")].clear(),
                _ => {}
            }
        } else {
            screen[row.expect("a row to write to")].push(c);
        }
    }
}

fn rows_at(frames: &[Value], frame: u64) -> Vec<String> {
    let shown = frames.iter().position(|entry| number(entry) == frame).expect("the frame is on the timeline");
    let key = frames[..=shown].iter().rposition(|entry| entry["keyframe"] == true).expect("a keyframe before it");
    let height = frames[shown]["rows"].as_u64().expect("the frame's rows") as usize;
    let mut screen = vec![String::new(); height];
    for entry in &frames[key..=shown] {
        redraw(&mut screen, entry["paint"].as_str().expect("a paint"));
    }
    let mut rows: Vec<String> = screen.iter().map(|row| row.trim_end().to_string()).collect();
    let used = rows.iter().rposition(|row| !row.is_empty()).map_or(0, |last| last + 1);
    rows.truncate(used);
    rows
}

#[test]
fn the_timeline_serves_keyframes_and_every_frame_after_them_redrawing_what_snapshot_showed() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    let before = now_ms();
    at.run(&[
        "open",
        "--size",
        "20x3",
        "--wait",
        "line 1",
        &fixture("line-printer"),
        "--lines",
        "1",
        "--cue",
        &cue.path(),
    ])
    .success();
    let mut snapshots = vec![snapshot_rows(&at, &[])];
    for n in 2..=40 {
        cue.print(&format!("cue {n}\n"));
        snapshots.push(snapshot_rows(&at, &[]));
    }
    let after = now_ms();
    let last = snapshots.last().expect("snapshots").1;
    assert!(last >= 40, "every cue drew a frame: {last}");

    let mut timeline = Timeline::open(dashboard(&at), "t1");
    let run = timeline.run_through(last);

    assert!((before..=after).contains(&run.started_ms), "{} not in {before}..={after}", run.started_ms);
    assert_eq!(run.frames.iter().map(number).collect::<Vec<_>>(), (1..=last).collect::<Vec<_>>());
    let keyframes: Vec<u64> = run.frames.iter().filter(|frame| frame["keyframe"] == true).map(number).collect();
    assert_eq!(keyframes, (1..=last).step_by(32).collect::<Vec<_>>());
    let times: Vec<u64> = run.frames.iter().map(|frame| frame["time_ms"].as_u64().expect("a time")).collect();
    assert!(times.windows(2).all(|pair| pair[0] <= pair[1]), "{times:?}");
    assert!(times.iter().all(|time| (before..=after).contains(time)), "{times:?} not in {before}..={after}");
    for frame in &run.frames {
        assert_eq!((frame["cols"].as_u64(), frame["rows"].as_u64()), (Some(20), Some(3)));
    }
    for (rows, frame) in &snapshots {
        assert_eq!(&rows_at(&run.frames, *frame), rows, "frame {frame}");
    }
    assert_eq!(rows_at(&run.frames, 1), ["line 1"]);
    assert_eq!(rows_at(&run.frames, last), ["cue 39", "cue 40"]);

    cue.print("cue 41\n");
    let (rows, frame) = snapshot_rows(&at, &[]);
    let more = timeline.next();
    assert_eq!(more["reset"], false, "{more}");
    let mut frames = run.frames;
    frames.extend(more["frames"].as_array().expect("frames").iter().cloned());
    assert_eq!(frames.iter().map(number).collect::<Vec<_>>(), (1..=frame).collect::<Vec<_>>());
    assert_eq!(rows_at(&frames, frame), rows);
}

#[test]
fn a_resize_draws_a_keyframe_at_the_new_size() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    at.run(&[
        "open",
        "--size",
        "20x3",
        "--wait",
        "line 3",
        &fixture("line-printer"),
        "--lines",
        "3",
        "--cue",
        &cue.path(),
    ])
    .success();
    let narrow = snapshot_rows(&at, &[]);
    at.run(&["resize", "24x5"]).success();
    let resized = snapshot_rows(&at, &[]);
    cue.print("wider\n");
    let wider = snapshot_rows(&at, &[]);
    assert_eq!(wider.0, ["line 1", "line 2", "line 3", "wider"]);
    assert!(resized.1 > narrow.1, "the resize drew frame {} after {}", resized.1, narrow.1);

    let mut timeline = Timeline::open(dashboard(&at), "t1");
    let run = timeline.run_through(wider.1);

    let drawn_by_resize = &run.frames[narrow.1 as usize];
    assert_eq!(drawn_by_resize["keyframe"], true, "{drawn_by_resize}");
    assert_eq!((drawn_by_resize["cols"].as_u64(), drawn_by_resize["rows"].as_u64()), (Some(24), Some(5)));
    for (rows, frame) in [&narrow, &resized, &wider] {
        assert_eq!(&rows_at(&run.frames, *frame), rows, "frame {frame}");
    }
}

#[test]
fn after_a_restart_the_timeline_starts_again_with_the_new_run_and_closes_with_the_tab() {
    let at = AgentTerminal::new();
    let cue = Cue::new();
    at.run(&[
        "open",
        "--size",
        "20x3",
        "--wait",
        "line 2",
        &fixture("line-printer"),
        "--lines",
        "2",
        "--cue",
        &cue.path(),
    ])
    .success();
    cue.print("before\n");
    let (_, last) = snapshot_rows(&at, &[]);
    let mut timeline = Timeline::open(dashboard(&at), "t1");
    let first = timeline.run_through(last);
    assert_eq!(rows_at(&first.frames, last), ["line 2", "before"]);

    at.run(&["restart", "--wait", "line 2"]).success();
    cue.restarted();
    cue.print("after\n");
    let (rows, frame) = snapshot_rows(&at, &[]);
    assert_eq!(rows, ["line 2", "after"]);
    let second = timeline.run_through(frame);
    assert!(second.started_ms >= first.started_ms, "{} < {}", second.started_ms, first.started_ms);
    assert_eq!(second.frames.iter().map(number).collect::<Vec<_>>(), (1..=frame).collect::<Vec<_>>());
    assert_eq!(rows_at(&second.frames, frame), rows);

    at.run(&["close"]).success();
    assert_eq!(timeline.next(), serde_json::json!({ "type": "closed" }));
}
