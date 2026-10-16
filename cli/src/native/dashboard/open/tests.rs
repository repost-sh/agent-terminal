use std::path::PathBuf;

use serde_json::json;

use super::{Opening, under_home, working_directory};
use crate::protocol::{SessionInfo, Sessions};

fn opening(command: &str, shell_integration: bool) -> Opening {
    serde_json::from_value(json!({
        "session": "s",
        "new_session": true,
        "command": command,
        "cwd": "/srv",
        "cols": 30,
        "rows": 4,
        "shell_integration": shell_integration
    }))
    .unwrap()
}

fn asked(opening: Opening, open: &Sessions) -> String {
    match opening.request(open) {
        Ok(request) => {
            let mut sent = serde_json::to_value(request).unwrap();
            sent["env"] = json!("<inherited>");
            sent.to_string()
        }
        Err(refused) => format!("refused: {refused}"),
    }
}

#[test]
fn an_opening_becomes_an_open_or_a_shell_request() {
    let none = Sessions::default();
    insta::assert_snapshot!([asked(opening("htop -d 5", false), &none), asked(opening("/bin/zsh -l", true), &none)].join("\n"), @r#"
        {"args":["-d","5"],"command":"open","cwd":"/srv","env":"<inherited>","program":"htop","session":"s","size":{"cols":30,"rows":4},"timeout_ms":0,"wait":null}
        {"command":"shell","cwd":"/srv","env":"<inherited>","program":"/bin/zsh","session":"s","shell":"zsh","size":{"cols":30,"rows":4},"timeout_ms":0,"wait":null}
    "#);
}

#[test]
fn an_opening_is_refused_for_a_bad_command_size_session_or_shell() {
    let taken = Sessions { sessions: vec![SessionInfo::empty("s")] };
    let mut tiny = opening("htop", false);
    tiny.cols = 1;
    let mut again = opening("htop", false);
    again.new_session = false;
    let refusals = [
        asked(opening("htop 'unclosed", false), &Sessions::default()),
        asked(opening("   ", false), &Sessions::default()),
        asked(opening("htop", false), &taken),
        asked(tiny, &Sessions::default()),
        asked(opening("htop", true), &Sessions::default()),
    ];
    insta::assert_snapshot!(refusals.join("\n"), @r"
        refused: a quote in the command is not closed
        refused: enter a command
        refused: A session named s already exists.
        refused: columns and rows must be between 2 and 1000
        refused: htop has no shell integration; it comes with zsh, bash and fish
    ");
    assert!(asked(again, &taken).starts_with('{'));
}

#[test]
fn a_working_directory_is_absolute_or_under_home() {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap());
    assert_eq!(
        ["/srv", "  /tmp  ", "~", "~/work", "work", "~other"].map(|typed| working_directory(typed).unwrap()),
        [
            PathBuf::from("/srv"),
            PathBuf::from("/tmp"),
            home.clone(),
            home.join("work"),
            home.join("work"),
            home.join("~other")
        ]
    );
    assert_eq!([under_home("~"), under_home("~/a/b"), under_home("a"), under_home("~x")], ["", "a/b", "a", "~x"]);
}
