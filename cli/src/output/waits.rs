use crate::protocol::{RowRange, TextMatch, Wait};

pub(crate) fn describe_wait(wait: &Wait) -> String {
    match wait {
        Wait::Stable { quiet_ms } => format!("the screen to stay unchanged for {quiet_ms} ms"),
        Wait::Idle { quiet_ms } => format!("the program to print nothing for {quiet_ms} ms"),
        Wait::Elapsed { ms } => format!("{ms} ms to pass"),
        Wait::Exit { code: None } => "the program to exit".to_string(),
        Wait::Exit { code: Some(code) } => format!("the program to exit with code {code}"),
        Wait::Cursor { row, col } => format!("the cursor to reach {row},{col}"),
        Wait::Frame { frame } => format!("frame {frame}"),
        Wait::Command => "the shell command to finish".to_string(),
        Wait::Text(text) => describe_text(text, "to appear", "a match for"),
        Wait::Gone(text) => describe_text(text, "to disappear", "no match for"),
    }
}

fn describe_text(text: &TextMatch, plain: &str, pattern: &str) -> String {
    let what = if text.regex { format!("{pattern} /{}/", text.text) } else { format!("\"{}\" {plain}", text.text) };
    match text.rows {
        Some(RowRange { first, last }) => format!("{what} in rows {first}-{last}"),
        None => what,
    }
}

pub(crate) fn not_met(wait: &Wait) -> String {
    match wait {
        Wait::Stable { quiet_ms } => format!("the screen did not stay unchanged for {quiet_ms} ms"),
        Wait::Idle { quiet_ms } => format!("the program did not stop printing for {quiet_ms} ms"),
        Wait::Elapsed { ms } => format!("{ms} ms did not pass"),
        Wait::Exit { code: None } => "the program did not exit".to_string(),
        Wait::Exit { code: Some(code) } => format!("the program did not exit with code {code}"),
        Wait::Cursor { row, col } => format!("the cursor did not reach {row},{col}"),
        Wait::Frame { frame } => format!("the screen did not reach frame {frame}"),
        Wait::Command => "the shell command did not finish".to_string(),
        Wait::Text(text) => describe_text(text, "did not appear", "nothing matched"),
        Wait::Gone(text) => describe_text(text, "did not disappear", "still a match for"),
    }
}
