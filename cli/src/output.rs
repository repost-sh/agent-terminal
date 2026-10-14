use crate::protocol::{ErrorBody, Reply, SessionInfo, Snapshot};
use crate::session_id::SessionId;

pub fn screen(s: &Snapshot) -> String {
    let mut header = vec![format!("{} {}", s.session, s.tab), s.program.clone(), format!("{}x{}", s.cols, s.rows)];
    if s.alt_screen {
        header.push("alt-screen".into());
    }
    if let Some(mouse) = &s.mouse {
        header.push(format!("mouse:{mouse}"));
    }
    header.push(if s.cursor_visible {
        format!("cursor {},{}", s.cursor[0], s.cursor[1])
    } else {
        "cursor hidden".into()
    });
    header.push(format!("frame {}", s.frame));
    match (s.exited, s.exit_code) {
        (true, Some(code)) => header.push(format!("exited {code}")),
        (true, None) => header.push("exited".into()),
        (false, _) => {}
    }

    let mut out = header.join(" · ");
    out.push('\n');
    let width = s.lines.len().to_string().len();
    let shown = s.lines.iter().rposition(|line| !line.is_empty()).map_or(0, |i| i + 1);
    for (i, line) in s.lines[..shown].iter().enumerate() {
        out.push_str(&format!("{:>width$}│{line}\n", i + 1));
    }
    let total = s.lines.len();
    if shown < total {
        let blank =
            if shown + 1 == total { format!("row {total} blank") } else { format!("rows {}-{total} blank", shown + 1) };
        out.push_str(&format!("{:>width$}│ ({blank})\n", "…"));
    }
    out
}

pub fn reply(reply: &Reply, json: bool) {
    match reply {
        Reply::Snapshot(snapshot) if json => println!("{}", serde_json::json!(snapshot)),
        Reply::Snapshot(snapshot) => print!("{}", screen(snapshot)),
        Reply::Closed(closed) if json => println!("{}", serde_json::json!(closed)),
        Reply::Closed(closed) => {
            for tab in &closed.closed {
                println!("closed {} {}", tab.session, tab.tab);
            }
        }
        Reply::Sessions(sessions) if json => println!("{}", serde_json::json!(sessions)),
        Reply::Sessions(sessions) if sessions.sessions.is_empty() => println!("no sessions are open"),
        Reply::Sessions(sessions) => {
            for session in &sessions.sessions {
                print!("{}", tab_lines(session));
            }
        }
        Reply::Tabs(session) if json => println!("{}", serde_json::json!(session)),
        Reply::Tabs(session) if session.tabs.is_empty() => {
            println!("no tabs are open in session {}", session.session)
        }
        Reply::Tabs(session) => print!("{}", tab_lines(session)),
        Reply::Stopped => {}
    }
}

fn tab_lines(session: &SessionInfo) -> String {
    let mut out = String::new();
    for tab in &session.tabs {
        let mut line =
            vec![format!("{} {}", session.session, tab.tab), tab.program.clone(), format!("{}x{}", tab.cols, tab.rows)];
        line.push(match (tab.exited, tab.exit_code) {
            (true, Some(code)) => format!("exited {code}"),
            (true, None) => "exited".into(),
            (false, _) => "running".into(),
        });
        if session.current.as_ref() == Some(&tab.tab) {
            line.push("current".into());
        }
        out.push_str(&line.join(" · "));
        out.push('\n');
    }
    out
}

pub fn session(name: &str, json: bool) {
    if json {
        println!("{}", serde_json::json!({ "session": name }));
    } else {
        println!("{name}");
    }
}

pub fn session_id(id: &SessionId, json: bool) {
    if json {
        println!("{}", serde_json::json!(id));
    } else {
        println!("{}", id.session);
    }
}

pub fn error(error: &ErrorBody, json: bool) -> i32 {
    if json {
        println!("{}", serde_json::json!({ "error": error }));
    } else {
        eprintln!("error: {}", error.message);
        if let Some(snapshot) = &error.snapshot {
            eprint!("{}", screen(snapshot));
        }
    }
    error.code
}

pub fn fail(message: &str, code: i32) -> i32 {
    eprintln!("error: {message}");
    code
}
