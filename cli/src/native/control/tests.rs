use std::sync::Arc;
use std::time::Duration;

use super::{Control, Handed};
use crate::native::changes::Changes;
use crate::protocol::Owner;

fn handed(control: &Control, timeout: Duration) -> &'static str {
    match control.wait_for_agent(timeout) {
        Handed::Back => "back",
        Handed::TimedOut => "timed out",
        Handed::Closed => "closed",
    }
}

fn state(control: &Control) -> (Owner, bool, &'static str) {
    (control.owner(), control.agent().is_some(), handed(control, Duration::ZERO))
}

#[test]
fn a_human_holds_the_tab_until_the_last_hold_is_dropped_and_each_change_is_announced() {
    let changes = Changes::default();
    let announced = changes.subscribe();
    let control = Control::new(changes);
    assert_eq!(state(&control), (Owner::Agent, true, "back"));
    let first = control.take();
    let second = control.take();
    assert_eq!(state(&control), (Owner::Human, false, "timed out"));
    drop(first);
    assert_eq!(state(&control), (Owner::Human, false, "timed out"));
    drop(second);
    assert_eq!((state(&control), *announced.borrow()), ((Owner::Agent, true, "back"), 4));
}

#[test]
fn a_hold_knows_the_control_it_holds() {
    let control = Control::new(Changes::default());
    let other = Control::new(Changes::default());
    let hold = control.take();
    assert_eq!([hold.holds(&control), hold.holds(&other)], [true, false]);
}

#[test]
fn a_wait_for_the_agent_ends_when_the_human_hands_back_or_the_tab_closes() {
    let control = Control::new(Changes::default());
    let waiting = |control: &Arc<Control>| {
        let control = Arc::clone(control);
        std::thread::spawn(move || handed(&control, Duration::from_secs(60)))
    };
    let hold = control.take();
    let handing_back = waiting(&control);
    drop(hold);
    assert_eq!(handing_back.join().unwrap(), "back");
    let _held = control.take();
    let closing = waiting(&control);
    control.close();
    assert_eq!([closing.join().unwrap(), handed(&control, Duration::ZERO)], ["closed", "closed"]);
}
