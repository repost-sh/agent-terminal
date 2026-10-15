use super::{Browser, Dashboard, snapshot_rows};
use crate::support::AgentTerminal;

const FULL_ROW_NARROW_IN_THE_DAEMON_WIDE_IN_XTERMS_DEFAULT_TABLE: &str = "㉈yyyyyyyyyyyyyyyyyyy";

#[test]
fn wide_and_combining_characters_take_the_same_columns_as_in_the_daemon() {
    let at = AgentTerminal::new();
    let script = format!(
        r#"printf "\033[1;1H✅\033[1;3Hx\033[2;1H🫠\033[2;3Hx\033[3;1H漢\033[3;3Hx\033[4;1He\314\201\033[4;2Hx\033[5;1H{FULL_ROW_NARROW_IN_THE_DAEMON_WIDE_IN_XTERMS_DEFAULT_TABLE}\033[6;1Hax"; sleep 300"#
    );
    at.run(&["open", "--size", "20x6", "--wait", "ax", "--", "sh", "-c", &script]).success();
    let expected = ["✅x", "🫠x", "漢x", "e\u{301}x", FULL_ROW_NARROW_IN_THE_DAEMON_WIDE_IN_XTERMS_DEFAULT_TABLE, "ax"];

    let (rows, _) = snapshot_rows(&at, &[]);
    assert_eq!(rows, expected);

    let dashboard = Dashboard::start(&at);
    let browser = Browser::open(&dashboard.url("/s/default/t1"));
    browser.wait_for_screen("ax");
    assert_eq!(browser.screen(), expected);
}
