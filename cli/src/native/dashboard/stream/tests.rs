use std::time::Duration;

use tokio::time::Instant;

use super::{Pace, Shown};
use crate::native::dashboard::follow::Wake;
use crate::native::terminal::{Glance, Status};
use crate::protocol::Size;

const THIRTY_A_SECOND: Duration = Duration::from_millis(33);

fn glance(frame: u64) -> Glance {
    Glance {
        frame,
        size: Size { cols: 80, rows: 24 },
        alt_screen: false,
        mouse: None,
        bracketed_paste: false,
        status: Status::Running,
    }
}

fn showing(frame: u64, sent: Instant, in_flight: usize) -> Pace {
    Pace { in_flight, last_sent: Some(sent), shown: Some(Shown { glance: glance(frame), rows: Vec::new() }) }
}

#[test]
fn the_first_frame_is_sent_at_once() {
    assert_eq!(Pace::default().wake(glance(1), Instant::now()), None);
}

#[test]
fn a_changed_screen_thirty_three_milliseconds_or_more_after_the_last_send_is_sent_now() {
    let sent = Instant::now();
    let pace = showing(1, sent, 1);

    assert_eq!(pace.wake(glance(2), sent + THIRTY_A_SECOND), None);
    assert_eq!(pace.wake(glance(2), sent + Duration::from_secs(5)), None);
}

#[test]
fn a_changed_screen_sooner_waits_until_thirty_three_milliseconds_after_the_last_send() {
    let sent = Instant::now();
    let pace = showing(1, sent, 1);

    assert_eq!(pace.wake(glance(2), sent), Some(Wake::At(sent + THIRTY_A_SECOND)));
    assert_eq!(pace.wake(glance(2), sent + Duration::from_millis(32)), Some(Wake::At(sent + THIRTY_A_SECOND)));
}

#[test]
fn with_two_frames_unacknowledged_it_waits_for_a_message_whatever_the_time() {
    let sent = Instant::now();
    let pace = showing(1, sent, 2);

    assert_eq!(pace.wake(glance(2), sent), Some(Wake::OnMessage));
    assert_eq!(pace.wake(glance(2), sent + Duration::from_secs(5)), Some(Wake::OnMessage));
}

#[test]
fn an_acknowledgement_reopens_the_window_and_any_other_message_does_not() {
    let sent = Instant::now();
    let mut pace = showing(1, sent, 2);

    pace.received(r#"{"type":"resize"}"#);
    assert_eq!(pace.wake(glance(2), sent + THIRTY_A_SECOND), Some(Wake::OnMessage));

    pace.received(r#"{"type":"shown"}"#);
    assert_eq!(pace.wake(glance(2), sent + THIRTY_A_SECOND), None);
}

#[test]
fn an_unchanged_screen_waits_for_a_change() {
    let sent = Instant::now();
    let pace = showing(1, sent, 1);

    assert_eq!(pace.wake(glance(1), sent + Duration::from_secs(5)), Some(Wake::OnChange));
}
