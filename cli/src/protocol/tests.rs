use std::io::{self, Read};

use serde_json::json;

use super::{
    Addressed, Cell, ClickTarget, CommandRecord, Direction, ErrorBody, Modes, MouseButton, MouseEvent, Name,
    OtherVersion, Reply, Request, ScreenshotFormat, Shell, Size, Snapshot, Squeeze, Style, TextMatch, Theme, read,
    read_body, write,
};

struct FailingOnce<'a> {
    error: Option<io::ErrorKind>,
    rest: &'a [u8],
}

impl Read for FailingOnce<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.error.take() {
            Some(kind) => Err(kind.into()),
            None => self.rest.read(buf),
        }
    }
}

fn cell(bold: bool, inverse: bool, underline: bool) -> Cell {
    Cell { row: 1, col: 1, char: "a".into(), fg: "red".into(), bg: "blue".into(), bold, inverse, underline }
}

#[test]
fn a_frame_is_a_big_endian_length_then_the_json_body() {
    let mut sent = Vec::new();
    write(&mut sent, &json!({ "a": 1 })).unwrap();
    assert_eq!(sent, b"\0\0\0\x07{\"a\":1}");
    let mut stream = sent.as_slice();
    assert_eq!(read::<serde_json::Value>(&mut stream).unwrap(), Some(json!({ "a": 1 })));
    assert_eq!(read::<serde_json::Value>(&mut stream).unwrap(), None);
}

#[test]
fn a_read_retries_an_interrupted_read_and_reports_any_other_failure() {
    let frame = b"\0\0\0\x02{}";
    let mut interrupted = FailingOnce { error: Some(io::ErrorKind::Interrupted), rest: frame };
    assert_eq!(read_body(&mut interrupted).unwrap(), Some(b"{}".to_vec()));
    let mut broken = FailingOnce { error: Some(io::ErrorKind::BrokenPipe), rest: frame };
    assert_eq!(read_body(&mut broken).unwrap_err().to_string(), "read message length");
    assert_eq!(read_body(&mut [0u8, 0].as_slice()).unwrap_err().to_string(), "read message length");
}

#[test]
fn a_frame_longer_than_64_mib_is_refused_before_its_body_is_read() {
    let refusal = |len: u32| read_body(&mut len.to_be_bytes().as_slice()).unwrap_err().to_string();
    assert_eq!(refusal(64 * 1024 * 1024 + 1), "message too large: 67108865 bytes");
    assert_eq!(refusal(64 * 1024 * 1024), "read message");
}

#[test]
fn a_click_target_is_a_ref_a_cell_counted_from_one_or_text() {
    let parsed = |text: &str| text.parse::<ClickTarget>();
    assert_eq!(
        ["@t3", "@t0", "4,12", "a,b", "Save"].map(parsed),
        [
            Ok(ClickTarget::Ref(3)),
            Ok(ClickTarget::Text("@t0".into())),
            Ok(ClickTarget::Cell { row: 4, col: 12 }),
            Ok(ClickTarget::Text("a,b".into())),
            Ok(ClickTarget::Text("Save".into()))
        ]
    );
    assert_eq!(
        ["0,3", "3,0", ""].map(parsed),
        [
            Err("rows and columns count from 1".into()),
            Err("rows and columns count from 1".into()),
            Err("give a ref (@t3), a cell (ROW,COL) or the text to click".into())
        ]
    );
    let targets = [ClickTarget::Ref(3), ClickTarget::Cell { row: 4, col: 12 }, ClickTarget::Text("Save".into())];
    assert_eq!(serde_json::to_value(targets).unwrap(), json!(["@t3", "4,12", "Save"]));
}

#[test]
fn a_mouse_event_is_at_its_cell() {
    let events = [
        MouseEvent::Move { row: 1, col: 2 },
        MouseEvent::Down { row: 3, col: 4, button: MouseButton::Left },
        MouseEvent::Up { row: 5, col: 6, button: MouseButton::Right },
        MouseEvent::Wheel { row: 7, col: 8, direction: Direction::Down },
    ];
    assert_eq!(events.map(MouseEvent::at), [(1, 2), (3, 4), (5, 6), (7, 8)]);
}

#[test]
fn a_mode_switch_crosses_the_socket_as_a_bool() {
    let sent = json!({
        "alt_screen": true,
        "mouse": null,
        "bracketed_paste": false,
        "app_cursor": true,
        "focus_events": false,
        "keyboard": []
    });
    let modes: Modes = serde_json::from_value(sent.clone()).unwrap();
    assert_eq!(serde_json::to_value(&modes).unwrap(), sent);
}

#[test]
fn a_command_failed_once_it_ended_with_a_nonzero_exit_code() {
    let failed = |ended_at_ms: Option<u64>, exit_code: Option<i32>| {
        let record: CommandRecord = serde_json::from_value(json!({
            "id": 1,
            "command": "make",
            "cwd": "/home/me/work",
            "started_at_ms": 0,
            "ended_at_ms": ended_at_ms,
            "duration_ms": 0,
            "exit_code": exit_code,
            "first_frame": 0,
            "last_frame": null
        }))
        .unwrap();
        record.failed()
    };
    assert_eq!(
        [failed(None, Some(1)), failed(Some(5), Some(0)), failed(Some(5), None), failed(Some(5), Some(2))],
        [false, false, false, true]
    );
}

#[test]
fn an_error_carries_the_screen_or_the_other_versions_it_met() {
    let screen: Snapshot = serde_json::from_value(json!({
        "session": "default",
        "tab": "app",
        "program": "bash",
        "cols": 80,
        "rows": 24,
        "alt_screen": false,
        "mouse": null,
        "cursor": [0, 2],
        "cursor_visible": true,
        "frame": 3,
        "exited": false,
        "exit_code": null,
        "first_row": 0,
        "lines": ["$ "]
    }))
    .unwrap();
    let failure = serde_json::to_value(ErrorBody::failure("not yet").with_screen(screen)).unwrap();
    assert_eq!(
        (&failure["code"], &failure["message"], &failure["snapshot"]["lines"]),
        (&json!(1), &json!("not yet"), &json!(["$ "]))
    );
    let versions =
        OtherVersion { cli_version: Some("1.2.0".into()), daemon_version: Some("1.1.0".into()), terminals: Some(2) };
    assert_eq!(
        serde_json::to_value(ErrorBody::other_version("older daemon", versions)).unwrap(),
        json!({
            "code": 6,
            "message": "older daemon",
            "other_version": { "cli_version": "1.2.0", "daemon_version": "1.1.0", "terminals": 2 }
        })
    );
}

#[test]
fn a_cell_reads_as_its_character_colours_and_the_styles_it_has() {
    assert_eq!(
        [cell(true, false, true).to_string(), cell(false, true, false).to_string()],
        ["\"a\" fg=red bg=blue bold underline", "\"a\" fg=red bg=blue inverse"]
    );
}

fn exit_code(reply: serde_json::Value) -> i32 {
    serde_json::from_value::<Reply>(reply).unwrap().exit_code()
}

fn screenshot_diff(passed: bool) -> serde_json::Value {
    json!({
        "reply": "ScreenshotDiff",
        "data": {
            "session": "default",
            "tab": "app",
            "baseline": "/home/me/work/before.png",
            "path": "/home/me/work/diff.png",
            "frame": 4,
            "changed": 10,
            "pixels": 100,
            "threshold": 0.01,
            "passed": passed
        }
    })
}

#[test]
fn a_batch_exits_with_the_code_of_its_first_failing_step() {
    let step = |response: serde_json::Value| json!({ "args": ["press", "x"], "response": response });
    let batch = |steps: &[serde_json::Value]| exit_code(json!({ "reply": "Batch", "data": steps }));
    let stopped = step(json!({ "Ok": { "reply": "Stopped" } }));
    let failed = |code: i32| step(json!({ "Err": { "code": code, "message": "no" } }));
    assert_eq!(
        [
            batch(&[stopped.clone(), step(json!(null)), step(json!({ "Ok": screenshot_diff(false) })), failed(5)]),
            batch(&[stopped.clone(), failed(3), failed(5)]),
            batch(&[stopped])
        ],
        [2, 3, 0]
    );
}

#[test]
fn a_checked_command_exits_with_its_own_code_and_a_failed_screenshot_diff_with_an_assertion() {
    let command = |check: bool, exit_code: Option<i32>| {
        json!({
            "reply": "Command",
            "data": {
                "session": "default",
                "tab": "app",
                "id": 1,
                "command": "make",
                "cwd": "/home/me/work",
                "started_at_ms": 0,
                "ended_at_ms": 5,
                "duration_ms": 5,
                "exit_code": exit_code,
                "first_frame": 0,
                "last_frame": 2,
                "output": "",
                "check": check
            }
        })
    };
    assert_eq!(
        [
            command(true, Some(4)),
            command(true, None),
            command(false, Some(4)),
            screenshot_diff(false),
            screenshot_diff(true)
        ]
        .map(exit_code),
        [4, 1, 0, 2, 0]
    );
}

#[test]
fn a_screenshot_format_and_theme_are_named_in_any_case() {
    let names = ScreenshotFormat::ALL.map(|format| format.to_string());
    assert_eq!(names, ["png", "svg", "html", "txt", "ansi"]);
    assert_eq!(names.map(|name| name.to_uppercase().parse()), ScreenshotFormat::ALL.map(Ok));
    assert_eq!("gif".parse::<ScreenshotFormat>(), Err("unknown format gif; use png, svg, html, txt or ansi".into()));
    assert_eq!(
        ["Light", "dark", "dim"].map(str::parse::<Theme>),
        [Ok(Theme::Light), Ok(Theme::Dark), Err("unknown theme dim; use dark or light".into())]
    );
}

#[test]
fn only_a_snapshot_asked_with_squeeze_blank_is_squeezed_and_a_batch_squeezes_step_by_step() {
    let options = |squeeze_blank: bool| json!({ "rows": null, "scrollback": "none", "ansi": false, "refs": false, "squeeze_blank": squeeze_blank });
    let snapshot = |squeeze_blank| json!({ "command": "snapshot", "session": "w", "tab": null, "options": options(squeeze_blank) });
    let typed = json!({ "command": "input", "session": "w", "tab": null, "input": { "kind": "type", "text": "x", "delay_ms": 0 }, "wait": null, "snapshot": options(true), "timeout_ms": 1 });
    let steps = [snapshot(true), snapshot(false), json!({ "command": "sessions" })];
    let batch = json!({ "command": "batch", "bail": false, "steps": steps.map(|request| json!({ "args": [], "request": request })) });
    let squeeze = |request: serde_json::Value| serde_json::from_value::<Request>(request).unwrap().squeeze();
    assert_eq!(
        [snapshot(true), snapshot(false), typed, batch].map(squeeze),
        [Squeeze::On, Squeeze::Off, Squeeze::On, Squeeze::PerStep(vec![Squeeze::On, Squeeze::Off, Squeeze::Off])]
    );
}

#[test]
fn a_request_addresses_a_tab_by_its_name() {
    let addressed = |request: serde_json::Value| match serde_json::from_value::<Request>(request) {
        Ok(request) => match request.addressed() {
            Addressed::Tab(session, tab) => format!("{session} {}", tab.map_or("(current)", |tab| &**tab)),
            Addressed::Session(session) => session.to_string(),
            Addressed::Nothing => "nothing".to_string(),
        },
        Err(refused) => refused.to_string(),
    };
    assert_eq!(
        [
            addressed(json!({ "command": "snapshot", "session": "work", "tab": "api" })),
            addressed(json!({ "command": "rename-tab", "session": "work", "tab": null, "name": "web" })),
            addressed(json!({ "command": "switch-tab", "session": "work", "tab": "web" })),
            addressed(json!({ "command": "snapshot", "session": "work", "tab": "T1" })),
        ],
        [
            "work api",
            "work (current)",
            "work web",
            "\"T1\" is not a name: a name is 1 to 24 characters of a-z, 0-9 and -, starting with a letter or digit, such as repost-forward"
        ]
    );
}

#[test]
fn a_size_is_two_to_a_thousand_cells_each_way() {
    let refused = Err("columns and rows must be between 2 and 1000".to_string());
    assert_eq!(
        [Size::new(2, 1000), Size::new(1000, 2), Size::new(1, 24), Size::new(80, 1001)],
        [Ok(Size { cols: 2, rows: 1000 }), Ok(Size { cols: 1000, rows: 2 }), refused.clone(), refused]
    );
}

#[test]
fn a_shell_is_known_by_its_programs_file_name_even_as_a_login_shell() {
    assert_eq!(
        ["/bin/zsh", "-bash", "/usr/local/bin/fish", "sh", "/bin/zsh5", ""].map(Shell::of_program),
        [Some(Shell::Zsh), Some(Shell::Bash), Some(Shell::Fish), None, None, None]
    );
}

#[test]
fn a_text_match_reads_quoted_or_as_a_regex_between_slashes() {
    let shown = |regex| TextMatch { text: "a.b".into(), regex, rows: None }.to_string();
    assert_eq!([shown(false), shown(true)], ["\"a.b\"", "/a.b/"]);
}

#[test]
fn a_style_is_a_flag_or_a_colour_in_any_case_and_reads_back_as_written() {
    let styles = ["Bold", "INVERSE", "underline", "fg=Red", "BG=blue"].map(|text| text.parse::<Style>().unwrap());
    assert_eq!(styles.map(|style| style.to_string()), ["bold", "inverse", "underline", "fg=red", "bg=blue"]);
    assert_eq!(
        ["italic", "fg=", "size=3"].map(|text| text.parse::<Style>().unwrap_err()),
        [
            "unknown style italic; use bold, inverse, underline, fg=<color> or bg=<color>",
            "unknown style fg=; use bold, inverse, underline, fg=<color> or bg=<color>",
            "unknown style size=3; use bold, inverse, underline, fg=<color> or bg=<color>"
        ]
    );
}

#[test]
fn a_cell_holds_a_style_when_it_has_that_flag_or_colour() {
    let styles = ["bold", "inverse", "underline", "fg=red", "fg=blue", "bg=blue", "bg=red"]
        .map(|text| text.parse::<Style>().unwrap());
    let held = |cell: &Cell| styles.clone().map(|style| style.holds(cell));
    assert_eq!(
        [held(&cell(true, false, false)), held(&cell(false, true, true))],
        [[true, false, false, true, false, true, false], [false, true, true, true, false, true, false]]
    );
}

#[test]
fn a_name_is_1_to_24_lowercase_letters_digits_and_dashes_starting_with_a_letter_or_digit() {
    let named = |text: &str| text.parse::<Name>().is_ok();
    assert_eq!(
        ["repost-forward", "9lives", "a", "x-", "abcdefghijklmnopqrstuvwx"].map(named),
        [true, true, true, true, true]
    );
    assert_eq!(
        ["", "-x", "Repost", "a_b", "a b", "abcdefghijklmnopqrstuvwxy", "caf\u{e9}"].map(named),
        [false, false, false, false, false, false, false]
    );
    assert_eq!(
        "Repost".parse::<Name>().err(),
        Some(
            "\"Repost\" is not a name: a name is 1 to 24 characters of a-z, 0-9 and -, starting with a letter or digit, such as repost-forward"
                .to_string()
        )
    );
    let sent: Name = serde_json::from_value(json!("repost-forward")).unwrap();
    assert_eq!(
        (&*sent, serde_json::to_value(&sent).unwrap(), serde_json::from_value::<Name>(json!("Bad")).is_err()),
        ("repost-forward", json!("repost-forward"), true)
    );
}
