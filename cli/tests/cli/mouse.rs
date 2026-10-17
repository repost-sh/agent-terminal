use serde_json::json;

use crate::support::{AgentTerminal, fixture};

const NO_MOUSE: &str = "this app has no mouse support; use keys";

fn open_echo(at: &AgentTerminal, size: &str, modes: &[&str]) {
    let echo = fixture("input-echo");
    let mut args = vec!["open", "--name", "app", "--size", size, "--wait", "ready", echo.as_str()];
    args.extend_from_slice(modes);
    at.run(&args).success();
}

#[test]
fn click_sends_a_press_and_a_release_in_the_sgr_encoding() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &["--mouse", "click", "--mouse-encoding", "sgr"]);
    let clicked = at.run(&["click", "2,5", "--wait", r"\e[<0;5;2M\e[<0;5;2m"]).success();
    assert_eq!(clicked.stdout(), "clicked 2,5 in work app\n");
    assert_eq!(at.received("work"), r"\e[<0;5;2M\e[<0;5;2m");
}

#[test]
fn click_right_and_double_send_the_right_button_twice() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &["--mouse", "click", "--mouse-encoding", "sgr"]);
    let expected = r"\e[<2;7;3M\e[<2;7;3m\e[<2;7;3M\e[<2;7;3m";
    let clicked = at.run(&["click", "3,7", "--right", "--double", "--wait", expected]).success();
    assert_eq!(clicked.stdout(), "clicked 3,7 in work app (right button, double click)\n");
    assert_eq!(at.received("work"), expected);
}

#[test]
fn click_and_mouse_events_carry_the_held_modifiers() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &["--mouse", "motion", "--mouse-encoding", "sgr"]);
    let clicked = at.run(&["click", "2,5", "--ctrl", "--wait", r"\e[<16;5;2M\e[<16;5;2m"]).success();
    assert_eq!(clicked.stdout(), "clicked 2,5 in work app (Ctrl held)\n");
    assert_eq!(
        at.run(&["mouse", "down", "3,4", "--alt", "--shift"]).success().stdout(),
        "mouse down at 3,4 in work app (Alt+Shift held)\n"
    );
    at.run(&["mouse", "move", "3,5", "--alt", "--shift"]).success();
    at.run(&["mouse", "up", "3,5", "--right", "--alt"]).success();
    let expected = r"\e[<16;5;2M\e[<16;5;2m\e[<12;4;3M\e[<44;5;3M\e[<10;5;3m\e[<80;5;3M";
    at.run(&["mouse", "wheel", "up", "3,5", "--ctrl", "--wait", expected]).success();
    assert_eq!(at.received("work"), expected);
}

#[test]
fn click_uses_the_x10_encoding_when_the_app_asks_for_no_other() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &["--mouse", "click"]);
    at.run(&["click", "2,5", "--wait", r#"\e[M %"\e[M#%""#]).success();
    assert_eq!(at.received("work"), r#"\e[M %"\e[M#%""#);
}

#[test]
fn click_uses_the_utf8_encoding_for_columns_past_95() {
    let at = AgentTerminal::new();
    open_echo(&at, "120x4", &["--mouse", "click", "--mouse-encoding", "utf8"]);
    let expected = r#"\e[M \xc2\x84"\e[M#\xc2\x84""#;
    at.run(&["click", "2,100", "--wait", expected]).success();
    assert_eq!(at.received("work"), expected);
}

#[test]
fn click_without_mouse_tracking_exits_1_and_sends_nothing() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &[]);
    let clicked = at.run(&["click", "2,5"]);
    assert_eq!(clicked.code(), 1, "{}", clicked.stderr());
    assert_eq!(
        clicked.stderr(),
        format!("error: work app: {NO_MOUSE} instead (`agent-terminal snapshot -i` lists the key hints on screen)\n")
    );
    at.run(&["press", "x", "--wait", "x"]).success();
    assert_eq!(at.received("work"), "x");
}

#[test]
fn click_on_a_cell_outside_the_screen_fails() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &["--mouse", "click", "--mouse-encoding", "sgr"]);
    let clicked = at.run(&["click", "9,5"]);
    assert_eq!(clicked.code(), 1);
    assert_eq!(clicked.stderr(), "error: work app: cell 9,5 is outside the 100x4 screen\n");
}

#[test]
fn click_on_text_clicks_its_first_match() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &["--mouse", "click", "--mouse-encoding", "sgr"]);
    let clicked = at.run(&["click", "ready", "--wait", r"\e[<0;1;1M\e[<0;1;1m"]).success();
    assert_eq!(clicked.stdout(), "clicked \"ready\" at 1,1 in work app\n");
    let missing = at.run(&["click", "nowhere"]);
    assert_eq!(missing.code(), 1);
    assert_eq!(missing.stderr(), "error: work app: \"nowhere\" is not on the screen\n");
}

#[test]
fn click_on_a_ref_clicks_where_the_ref_starts() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &["--mouse", "click", "--mouse-encoding", "sgr"]);
    at.run(&["wait", "--cursor", "2,1"]).success();
    let listed = at.run(&["snapshot", "-i"]).success();
    assert!(listed.stdout().ends_with("refs:\n  @t1 [input] \"\" row 2 col 1\n"), "{}", listed.stdout());
    let clicked = at.run(&["--json", "click", "@t1", "--wait", r"\e[<0;1;2M\e[<0;1;2m"]).success();
    assert_eq!(
        clicked.json(),
        serde_json::json!({
            "session": "work", "tab": "app", "input": "click", "target": "@t1",
            "row": 2, "col": 1, "button": "left", "double": false, "mods": { "shift": false, "alt": false, "ctrl": false }, "bytes": 18
        })
    );
}

#[test]
fn click_on_a_ref_whose_cells_changed_asks_for_a_new_snapshot() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &["--mouse", "click", "--mouse-encoding", "sgr"]);
    at.run(&["wait", "--cursor", "2,1"]).success();
    let frame = at.run(&["--json", "snapshot", "-i"]).success().json()["frame"].as_u64().expect("a frame");
    at.run(&["press", "x", "--wait", "x"]).success();
    let clicked = at.run(&["click", "@t1"]);
    assert_eq!(clicked.code(), 1, "{}", clicked.stderr());
    assert_eq!(
        clicked.stderr(),
        format!(
            "error: work app: @t1 is from frame {frame}, and that part of the screen has changed since; take a new `agent-terminal snapshot -i` and use its refs\n"
        )
    );
    assert_eq!(at.received("work"), "x");
}

#[test]
fn click_on_a_panel_whose_inside_changed_asks_for_a_new_snapshot() {
    let at = AgentTerminal::new();
    at.run(&["open", "--name", "app", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui"), "--mouse"])
        .success();
    let listed = at.run(&["--json", "snapshot", "-i"]).success().json();
    assert_eq!((&listed["refs"][0]["kind"], &listed["refs"][0]["label"]), (&json!("panel"), &json!("Files")));
    let frame = listed["frame"].as_u64().expect("a frame");

    at.run(&["press", "j", "--wait", "Selected: beta.txt"]).success();
    let clicked = at.run(&["click", "@t1"]);
    assert_eq!(clicked.code(), 1, "{}", clicked.stdout());
    assert_eq!(
        clicked.stderr(),
        format!(
            "error: work app: @t1 is from frame {frame}, and that part of the screen has changed since; take a new `agent-terminal snapshot -i` and use its refs\n"
        )
    );
}

#[test]
fn click_on_a_ref_no_snapshot_listed_fails() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &["--mouse", "click", "--mouse-encoding", "sgr"]);
    let clicked = at.run(&["click", "@t1"]);
    assert_eq!(clicked.code(), 1);
    assert_eq!(
        clicked.stderr(),
        "error: work app: there is no @t1 in the last `agent-terminal snapshot -i` of this tab; take one and use its refs\n"
    );
    at.run(&["wait", "--cursor", "2,1"]).success();
    at.run(&["snapshot", "-i"]).success();
    assert_eq!(at.run(&["click", "@t2"]).code(), 1);
}

#[test]
fn mouse_events_follow_motion_tracking() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &["--mouse", "motion", "--mouse-encoding", "sgr"]);
    assert_eq!(at.run(&["mouse", "move", "3,4"]).success().stdout(), "mouse move to 3,4 in work app\n");
    assert_eq!(at.run(&["mouse", "down", "3,4"]).success().stdout(), "mouse down at 3,4 in work app\n");
    at.run(&["mouse", "move", "3,6"]).success();
    assert_eq!(at.run(&["mouse", "up", "3,6"]).success().stdout(), "mouse up at 3,6 in work app\n");
    assert_eq!(at.run(&["mouse", "wheel", "up", "3,6"]).success().stdout(), "mouse wheel up at 3,6 in work app\n");
    let expected = r"\e[<35;4;3M\e[<0;4;3M\e[<32;6;3M\e[<0;6;3m\e[<64;6;3M\e[<65;6;3M";
    at.run(&["mouse", "wheel", "down", "3,6", "--wait", expected]).success();
    assert_eq!(at.received("work"), expected);
}

#[test]
fn mouse_motion_is_sent_only_when_the_tracking_reports_it() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &["--mouse", "drag", "--mouse-encoding", "sgr"]);
    assert_eq!(
        at.run(&["mouse", "move", "3,4"]).success().stdout(),
        "mouse move to 3,4 in work app (not sent: the app does not track this event)\n"
    );
    at.run(&["mouse", "down", "3,4", "--right"]).success();
    at.run(&["mouse", "move", "3,5"]).success();
    let expected = r"\e[<2;4;3M\e[<34;5;3M\e[<2;5;3m";
    at.run(&["mouse", "up", "3,5", "--right", "--wait", expected]).success();
    assert_eq!(at.received("work"), expected);
}

#[test]
fn mouse_with_click_tracking_sends_no_motion() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &["--mouse", "click", "--mouse-encoding", "sgr"]);
    at.run(&["mouse", "down", "3,4", "--middle"]).success();
    let moved = at.run(&["--json", "mouse", "move", "3,5"]).success().json();
    assert_eq!(moved["bytes"], 0);
    let expected = r"\e[<1;4;3M\e[<1;5;3m";
    at.run(&["mouse", "up", "3,5", "--middle", "--wait", expected]).success();
    assert_eq!(at.received("work"), expected);
}

#[test]
fn mouse_without_mouse_tracking_exits_1() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &[]);
    let moved = at.run(&["mouse", "down", "2,2"]);
    assert_eq!(moved.code(), 1);
    assert!(moved.stderr().contains(NO_MOUSE), "{}", moved.stderr());
}

#[test]
fn scroll_uses_the_wheel_in_the_middle_of_the_screen_when_the_app_tracks_the_mouse() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &["--mouse", "click", "--mouse-encoding", "sgr"]);
    let scrolled = at.run(&["scroll", "down", "2", "--wait", r"\e[<65;50;2M\e[<65;50;2M"]).success();
    assert_eq!(scrolled.stdout(), "scrolled down 2 in work app with the mouse wheel\n");
    let expected = r"\e[<65;50;2M\e[<65;50;2M\e[<64;50;2M";
    assert_eq!(
        at.run(&["scroll", "up", "--wait", expected]).success().stdout(),
        "scrolled up 1 in work app with the mouse wheel\n"
    );
    assert_eq!(at.received("work"), expected);
}

#[test]
fn scroll_presses_page_keys_when_the_app_does_not_track_the_mouse() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &[]);
    let scrolled = at.run(&["scroll", "down", "2", "--wait", r"\e[6~\e[6~"]).success();
    assert_eq!(scrolled.stdout(), "scrolled down 2 in work app with PageDown\n");
    let expected = r"\e[6~\e[6~\e[5~";
    assert_eq!(
        at.run(&["scroll", "up", "--wait", expected]).success().stdout(),
        "scrolled up 1 in work app with PageUp\n"
    );
    assert_eq!(at.received("work"), expected);
}

#[test]
fn scroll_keys_presses_page_keys_even_when_the_app_tracks_the_mouse() {
    let at = AgentTerminal::new();
    open_echo(&at, "100x4", &["--mouse", "click", "--mouse-encoding", "sgr"]);
    let scrolled = at.run(&["--json", "scroll", "down", "--keys", "--wait", r"\e[6~"]).success().json();
    assert_eq!(
        scrolled,
        serde_json::json!({
            "session": "work", "tab": "app", "input": "scroll", "direction": "down", "count": 1, "by": "keys", "bytes": 4
        })
    );
    assert_eq!(at.received("work"), r"\e[6~");
}

fn open_tui(at: &AgentTerminal, flags: &[&str]) {
    let tui = fixture("fixture-tui");
    let mut args = vec!["open", "--name", "app", "--size", "50x8", "--wait", "Quit: q", tui.as_str()];
    args.extend_from_slice(flags);
    at.run(&args).success();
}

#[test]
fn the_fixture_tui_without_mouse_refuses_clicks_and_scrolls_with_page_keys() {
    let at = AgentTerminal::new();
    open_tui(&at, &[]);
    let clicked = at.run(&["click", "gamma.txt"]);
    assert_eq!(clicked.code(), 1, "{}", clicked.stderr());
    assert!(clicked.stderr().contains(NO_MOUSE), "{}", clicked.stderr());
    let scrolled = at.run(&["scroll", "down", "--wait", "Selected: delta.txt"]).success();
    assert_eq!(scrolled.stdout(), "scrolled down 1 in work app with PageDown\n");
    at.run(&["scroll", "up", "--wait", "Selected: alpha.txt"]).success();
}

#[test]
fn the_fixture_tui_with_mouse_selects_the_clicked_row() {
    let at = AgentTerminal::new();
    open_tui(&at, &["--mouse"]);
    let header = at.run(&["snapshot"]).success().stdout();
    assert!(header.starts_with("work app · fixture-tui · 50x8 · alt-screen · mouse:sgr · "), "{header}");
    let clicked = at.run(&["click", "gamma.txt", "--wait", "Selected: gamma.txt"]).success();
    assert_eq!(clicked.stdout(), "clicked \"gamma.txt\" at 4,2 in work app\n");
    at.run(&["click", "beta.txt", "--right", "--wait", "Mouse: right 3,2"]).success();
    at.run(&["expect", "Selected: gamma.txt"]).success();
}

#[test]
fn the_fixture_tui_with_mouse_scrolls_its_list_with_the_wheel() {
    let at = AgentTerminal::new();
    open_tui(&at, &["--mouse"]);
    let scrolled = at.run(&["scroll", "down", "2", "--wait", "Selected: gamma.txt"]).success();
    assert_eq!(scrolled.stdout(), "scrolled down 2 in work app with the mouse wheel\n");
}

#[test]
fn the_fixture_tui_dialog_buttons_are_refs_to_click() {
    let at = AgentTerminal::new();
    open_tui(&at, &["--mouse"]);
    at.run(&["press", "d", "--wait", "[ Yes ]"]).success();
    insta::assert_snapshot!(at.run(&["snapshot", "-i"]).success().screen(), @r#"
    work app · fixture-tui · 50x8 · alt-screen · mouse:sgr · cursor hidden · frame N
    1│┌Files─────────────┐┌Delete──────────────────────┐
    2││alpha.txt         ││Delete alpha.txt?           │
    3││beta.txt          ││                            │
    4││gamma.txt         ││[ Yes ]  [ No ]             │
    5││delta.txt         ││                            │
    6││                  ││                            │
    7│└──────────────────┘└────────────────────────────┘
    8│Move: j/k | Quit: q | Crash: x
    refs:
      @t1 [panel]    "Files"     rows 1-7
      @t2 [panel]    "Delete"    rows 1-7
      @t3 [selected] "alpha.txt" row 2 col 2
      @t4 [button]   "Yes"       row 4 col 24
      @t5 [button]   "No"        row 4 col 33
      @t6 [hint]     "Move: j/k" row 8 col 1
      @t7 [hint]     "Quit: q"   row 8 col 13
      @t8 [hint]     "Crash: x"  row 8 col 23
    "#);
    let clicked = at.run(&["click", "@t4", "--wait", "Deleted: alpha.txt"]).success();
    assert_eq!(clicked.stdout(), "clicked @t4 at 4,24 in work app\n");
    let stale = at.run(&["click", "@t5"]);
    assert_eq!(stale.code(), 1, "{}", stale.stderr());
    assert!(stale.stderr().contains("@t5 is from frame "), "{}", stale.stderr());
}

#[test]
fn a_ref_of_the_fixture_tui_works_while_its_cells_are_unchanged() {
    let at = AgentTerminal::new();
    open_tui(&at, &["--mouse"]);
    at.run(&["snapshot", "-i"]).success();
    at.run(&["press", "j", "--wait", "Selected: beta.txt"]).success();
    for changed in ["@t1", "@t2", "@t3"] {
        let stale = at.run(&["click", changed]);
        assert_eq!(stale.code(), 1, "{changed}: {}", stale.stderr());
        assert!(stale.stderr().contains("take a new `agent-terminal snapshot -i`"), "{}", stale.stderr());
    }
    let clicked = at.run(&["click", "@t4", "--wait", "Mouse: left 8,1"]).success();
    assert_eq!(clicked.stdout(), "clicked @t4 at 8,1 in work app\n");
    at.run(&["expect", "Selected: beta.txt"]).success();
}

#[test]
fn the_fixture_tui_dialog_answers_y_n_and_esc_from_the_keyboard() {
    let at = AgentTerminal::new();
    open_tui(&at, &[]);
    for (answer, status) in [("y", "Deleted: alpha.txt"), ("n", "Kept: alpha.txt"), ("Esc", "Kept: alpha.txt")] {
        at.run(&["press", "d", "--wait", "[ Yes ]"]).success();
        at.run(&["press", "j"]).success();
        at.run(&["press", answer, "--wait-gone", "[ Yes ]"]).success();
        at.run(&["expect", status]).success();
    }
}
