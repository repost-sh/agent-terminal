use super::screen;
use crate::output::saved_rows;
use crate::protocol::{Snapshot, Squeeze};

#[test]
fn a_squeezed_screen_folds_each_run_of_three_or_more_blank_rows_and_keeps_the_numbers() {
    let squeezed: Snapshot = serde_json::from_str(r#"{"alt_screen":false,"cols":30,"cursor":[3,1],"cursor_visible":true,"exit_code":null,"exited":false,"first_row":2,"frame":1,"lines":["a","","","","b","","","c","",""],"mouse":null,"program":"sh","rows":12,"scrollback":["","","","old"],"session":"rec","tab":"app"}"#).unwrap();
    insta::assert_snapshot!(screen(&squeezed, &Squeeze::On), @r##"
        rec app · sh · 30x12 · cursor 3,1 · frame 1
        -4│
        -3│
        -2│
        -1│old
         2│a
         …│ (rows 3-5 blank)
         6│b
         7│
         8│
         9│c
         …│ (rows 10-11 blank)
    "##);
}

fn squeezable(first_row: u16, lines: &[String]) -> Snapshot {
    let rows = usize::from(first_row) + lines.len();
    serde_json::from_value(serde_json::json!({
        "session": "rec", "tab": "app", "program": "sh", "cols": 30, "rows": rows, "alt_screen": false,
        "mouse": null, "cursor": [1, 1], "cursor_visible": true, "frame": 1, "exited": false, "exit_code": null,
        "first_row": first_row, "lines": lines
    }))
    .unwrap()
}

fn numbered_rows(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .filter_map(|line| line.split_once('│'))
        .filter_map(|(label, text)| Some((label.trim().parse().ok()?, text.to_string())))
        .collect()
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config { cases: 256, failure_persistence: None, ..proptest::test_runner::Config::default() })]

    #[test]
    fn squeezing_hides_only_runs_of_three_or_more_blank_rows(first_row in 1u16..20, filled in proptest::collection::vec(proptest::prelude::any::<bool>(), 0..30)) {
        let lines: Vec<String> = filled.iter().enumerate().map(|(i, &filled)| if filled { format!("row {i}") } else { String::new() }).collect();
        let squeezed = screen(&squeezable(first_row, &lines), &Squeeze::On);
        proptest::prop_assert_eq!(saved_rows(&squeezed), saved_rows(&screen(&squeezable(first_row, &lines), &Squeeze::Off)));
        let printed = numbered_rows(&squeezed);
        let first = usize::from(first_row);
        proptest::prop_assert!(printed.iter().all(|(row, text)| lines.get(row - first) == Some(text)));
        proptest::prop_assert!(printed.windows(2).all(|pair| pair[0].0 < pair[1].0));
        let blank_run = |run: &[(usize, String)]| run[2].0 == run[0].0 + 2 && run.iter().all(|(_, text)| text.is_empty());
        proptest::prop_assert!(!printed.windows(3).any(blank_run));
    }
}
