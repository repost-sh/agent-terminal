use crate::protocol::{
    BatchOutcome, Checked, ClickTarget, DashboardStopped, Delivered, Delivery, DiffBase, Direction, ErrorBody, Exit,
    Fact, Modes, MouseButton, MouseEvent, Ref, RefKind, Reply, Response, SavedRecording, ScrollBy, SessionInfo,
    Snapshot, SnapshotDiff, Waited,
};
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
    let scrollback = s.scrollback.as_deref().unwrap_or(&[]);
    let width =
        s.rows.to_string().len().max(if scrollback.is_empty() { 0 } else { scrollback.len().to_string().len() + 1 });
    for (i, line) in scrollback.iter().enumerate() {
        let label = format!("-{}", scrollback.len() - i);
        out.push_str(&format!("{label:>width$}│{line}\n"));
    }
    let first = s.first_row as usize;
    let shown = s.lines.iter().rposition(|line| !line.is_empty()).map_or(0, |i| i + 1);
    for (i, line) in s.lines[..shown].iter().enumerate() {
        out.push_str(&format!("{:>width$}│{line}\n", first + i));
    }
    let last = first + s.lines.len() - 1;
    if shown < s.lines.len() {
        let blank = if first + shown == last {
            format!("row {last} blank")
        } else {
            format!("rows {}-{last} blank", first + shown)
        };
        out.push_str(&format!("{:>width$}│ ({blank})\n", "…"));
    }
    if let Some(refs) = &s.refs {
        out.push_str(&refs_text(refs));
    }
    out
}

fn refs_text(refs: &[Ref]) -> String {
    if refs.is_empty() {
        return "refs: none\n".into();
    }
    let kinds: Vec<String> = refs.iter().map(|r| format!("[{}]", r.kind)).collect();
    let labels: Vec<String> = refs.iter().map(|r| format!("\"{}\"", r.label)).collect();
    let widest = |texts: &mut dyn Iterator<Item = usize>| texts.max().unwrap_or(0);
    let id_width = widest(&mut refs.iter().map(|r| r.id.chars().count()));
    let kind_width = widest(&mut kinds.iter().map(|kind| kind.chars().count()));
    let label_width = widest(&mut labels.iter().map(|label| label.chars().count()));
    let mut out = String::from("refs:\n");
    for ((r, kind), label) in refs.iter().zip(&kinds).zip(&labels) {
        let place = match r.kind {
            RefKind::Panel => format!("rows {}-{}", r.rows[0], r.rows[1]),
            _ => format!("row {} col {}", r.row, r.col),
        };
        out.push_str(&format!("  {:<id_width$} {kind:<kind_width$} {label:<label_width$} {place}\n", r.id));
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
        Reply::Delivered(delivered) if json => println!("{}", serde_json::json!(delivered)),
        Reply::Delivered(delivered) => {
            println!("{}", delivery(delivered));
            if let Some(snapshot) = &delivered.snapshot {
                print!("{}", screen(snapshot));
            }
        }
        Reply::Waited(waited) if json => println!("{}", serde_json::json!(waited)),
        Reply::Waited(Waited { session, tab, until, waited_ms, exit_code, .. }) => {
            let code = exit_code.map_or_else(String::new, |code| format!(" (exit code {code})"));
            println!("waited {waited_ms} ms for {until} in {session} {tab}{code}")
        }
        Reply::Checked(checked) if json => println!("{}", serde_json::json!(checked)),
        Reply::Checked(Checked { session, tab, fact }) => println!("{fact} in {session} {tab}"),
        Reply::Stopped => {}
        Reply::Batch(outcomes) if json => println!("{}", batch_json(outcomes)),
        Reply::Batch(outcomes) => batch(outcomes),
        Reply::Fact(fact) if json => println!("{}", serde_json::json!(fact)),
        Reply::Fact(fact) => print!("{}", fact_text(fact)),
        Reply::Matches(matches) if json => println!("{}", serde_json::json!(matches)),
        Reply::Matches(matches) => {
            for found in &matches.matches {
                println!("{},{} {}", found.row, found.col, found.text);
            }
        }
        Reply::Marked(marked) if json => println!("{}", serde_json::json!(marked)),
        Reply::Marked(marked) => println!("marked {} at frame {}", marked.mark, marked.frame),
        Reply::Printed(printed) if json => println!("{}", serde_json::json!(printed)),
        Reply::Printed(printed) => print!("{}", printed.output),
        Reply::Diff(diff) if json => println!("{}", serde_json::json!(diff)),
        Reply::Diff(diff) => print!("{}", diff_text(diff)),
        Reply::Recording(recording) if json => println!("{}", serde_json::json!(recording)),
        Reply::Recording(recording) => {
            println!("recording {} {} to {}", recording.session, recording.tab, recording.path.display())
        }
        Reply::Saved(saved) if json => println!("{}", serde_json::json!(saved)),
        Reply::Saved(saved) => println!("{}", saved_text(saved)),
        Reply::Exported(export) if json => println!("{}", serde_json::json!(export)),
        Reply::Exported(export) => println!(
            "exported session {} to {} ({}, {} KB)",
            export.session,
            export.path.display(),
            count(export.files, "file"),
            export.bytes.div_ceil(1024)
        ),
        Reply::Dashboard(dashboard) if json => println!("{}", serde_json::json!(dashboard)),
        Reply::Dashboard(dashboard) => println!("dashboard running at {}", dashboard.url),
        Reply::DashboardStopped(stopped) if json => println!("{}", serde_json::json!(stopped)),
        Reply::DashboardStopped(DashboardStopped { stopped: true }) => println!("dashboard stopped"),
        Reply::DashboardStopped(DashboardStopped { stopped: false }) => println!("no dashboard is running"),
    }
}

fn saved_text(saved: &SavedRecording) -> String {
    format!("saved {} ({}, {} KB)", saved.path.display(), duration(saved.duration_ms), saved.bytes.div_ceil(1024))
}

fn duration(ms: u64) -> String {
    match ms {
        0..1_000 => format!("{ms} ms"),
        1_000..60_000 => format!("{:.1} s", ms as f64 / 1000.0),
        _ => format!("{} min {:02} s", ms / 60_000, ms % 60_000 / 1000),
    }
}

fn diff_text(diff: &SnapshotDiff) -> String {
    let from = match &diff.since {
        DiffBase::Baseline(baseline) => baseline.clone(),
        DiffBase::Frame(frame) => format!("frame {frame}"),
    };
    let changed = match diff.changes.len() {
        0 => "no rows changed".to_string(),
        n => format!("{} changed", count(n, "row")),
    };
    let mut out = format!("{from} → frame {} · {changed}\n", diff.frame);
    let width = diff.changes.iter().map(|change| change.row.to_string().len()).max().unwrap_or(1);
    for change in &diff.changes {
        out.push_str(&format!("-{:>width$}│{}\n+{:>width$}│{}\n", change.row, change.before, change.row, change.after));
    }
    out
}

pub fn saved_rows(saved: &str) -> Option<Vec<String>> {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(saved) {
        let first = json.get("first_row").and_then(|first| first.as_u64()).unwrap_or(1) as usize;
        let lines = json.get("lines")?.as_array()?;
        let mut rows = vec![String::new(); first.saturating_sub(1)];
        rows.extend(lines.iter().map(|line| line.as_str().map(str::to_string)).collect::<Option<Vec<_>>>()?);
        return Some(rows);
    }
    let mut rows = Vec::new();
    let mut numbered = false;
    for (label, text) in saved.lines().filter_map(|line| line.split_once('│')) {
        numbered = true;
        if let Ok(row @ 1..) = label.trim().parse::<usize>() {
            if rows.len() < row {
                rows.resize(row, String::new());
            }
            rows[row - 1] = text.to_string();
        }
    }
    numbered.then_some(rows)
}

fn fact_text(fact: &Fact) -> String {
    match fact {
        Fact::Text(text) if text.is_empty() => String::new(),
        Fact::Text(text) => format!("{text}\n"),
        Fact::Cursor(cursor) if cursor.visible => format!("{},{}\n", cursor.row, cursor.col),
        Fact::Cursor(_) => "hidden\n".into(),
        Fact::Size(size) => format!("{}x{}\n", size.cols, size.rows),
        Fact::Frame(frame) => format!("{frame}\n"),
        Fact::Program(program) => format!("{program}\n"),
        Fact::ExitCode(Exit { exited: false, .. }) => "running\n".into(),
        Fact::ExitCode(Exit { code: Some(code), .. }) => format!("{code}\n"),
        Fact::ExitCode(Exit { code: None, .. }) => "exited, exit code unknown\n".into(),
        Fact::Pid(Some(pid)) => format!("{pid}\n"),
        Fact::Pid(None) => "unknown\n".into(),
        Fact::Title(Some(title)) => format!("{title}\n"),
        Fact::Title(None) => String::new(),
        Fact::Cwd(cwd) => format!("{cwd}\n"),
        Fact::Modes(modes) => modes_text(modes),
        Fact::Cell(cell) => format!("{cell}\n"),
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

fn delivery(d: &Delivered) -> String {
    let target = format!("{} {}", d.session, d.tab);
    match &d.delivery {
        Delivery::Text { keys, .. } => format!("typed {} into {target}", count(*keys, "key")),
        Delivery::Keys { keys, .. } => format!("pressed {} in {target}", count(*keys, "key")),
        Delivery::Paste { bracketed: true, bytes } => {
            format!("pasted {} into {target} (bracketed paste)", count(*bytes, "byte"))
        }
        Delivery::Paste { bracketed: false, bytes } => {
            format!("pasted {} into {target} (plain, the app has not turned on bracketed paste)", count(*bytes, "byte"))
        }
        Delivery::Raw { bytes } => format!("sent {} to {target}", count(*bytes, "raw byte")),
        Delivery::Signal { signal, process_group } => {
            format!("sent {signal} to {target} (process group {process_group})")
        }
        Delivery::Click { target: clicked, row, col, button, double, .. } => {
            let what = match clicked {
                ClickTarget::Cell { .. } => format!("{row},{col}"),
                ClickTarget::Ref(_) => format!("{clicked} at {row},{col}"),
                ClickTarget::Text(text) => format!("{text:?} at {row},{col}"),
            };
            let mut notes = button_note(*button);
            if *double {
                notes.push("double click".into());
            }
            format!("clicked {what} in {target}{}", noted(&notes))
        }
        Delivery::Mouse { event, bytes } => {
            let (what, mut notes) = match event {
                MouseEvent::Move { row, col } => (format!("mouse move to {row},{col}"), Vec::new()),
                MouseEvent::Down { row, col, button } => (format!("mouse down at {row},{col}"), button_note(*button)),
                MouseEvent::Up { row, col, button } => (format!("mouse up at {row},{col}"), button_note(*button)),
                MouseEvent::Wheel { row, col, direction } => {
                    (format!("mouse wheel {direction} at {row},{col}"), Vec::new())
                }
            };
            if *bytes == 0 {
                notes.push("not sent: the app does not track this event".into());
            }
            format!("{what} in {target}{}", noted(&notes))
        }
        Delivery::Scroll { direction, count, by, .. } => {
            let with = match (by, direction) {
                (ScrollBy::Wheel, _) => "the mouse wheel",
                (ScrollBy::Keys, Direction::Up) => "PageUp",
                (ScrollBy::Keys, Direction::Down) => "PageDown",
            };
            format!("scrolled {direction} {count} in {target} with {with}")
        }
    }
}

fn button_note(button: MouseButton) -> Vec<String> {
    match button {
        MouseButton::Left => Vec::new(),
        other => vec![format!("{other} button")],
    }
}

fn noted(notes: &[String]) -> String {
    if notes.is_empty() { String::new() } else { format!(" ({})", notes.join(", ")) }
}

fn modes_text(modes: &Modes) -> String {
    let on = |enabled: bool| if enabled { "on" } else { "off" };
    let mouse = modes.mouse.as_ref().map_or_else(|| "off".into(), |m| format!("{}, {}", m.tracking, m.encoding));
    let keyboard = if modes.keyboard.is_empty() { "legacy".into() } else { modes.keyboard.join(", ") };
    format!(
        "alt-screen: {}\nmouse: {mouse}\nbracketed-paste: {}\napp-cursor: {}\nfocus-events: {}\nkeyboard: {keyboard}\n",
        on(modes.alt_screen),
        on(modes.bracketed_paste),
        on(modes.app_cursor),
        on(modes.focus_events),
    )
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {noun}s") }
}

fn batch_json(outcomes: &[BatchOutcome]) -> serde_json::Value {
    outcomes
        .iter()
        .map(|outcome| {
            let mut entry = serde_json::json!({
                "args": outcome.args,
                "exit_code": outcome.response.as_ref().map(Response::exit_code),
            });
            match &outcome.response {
                Some(Response::Ok(ok)) => entry["result"] = serde_json::json!(ok)["data"].take(),
                Some(Response::Err(error)) => entry["error"] = serde_json::json!(error),
                None => {}
            }
            entry
        })
        .collect()
}

fn batch(outcomes: &[BatchOutcome]) {
    for (index, outcome) in outcomes.iter().enumerate() {
        if index > 0 {
            println!();
        }
        let call = format!("[{}] {}", index + 1, call_line(&outcome.args));
        let Some(response) = &outcome.response else {
            println!("{call} · not run");
            continue;
        };
        println!("{call} · exit {}", response.exit_code());
        match response {
            Response::Ok(ok) => reply(ok, false),
            Response::Err(error) => {
                println!("error: {}", error.message);
                if let Some(snapshot) = &error.snapshot {
                    print!("{}", screen(snapshot));
                }
            }
        }
    }
}

pub fn call_line(args: &[String]) -> String {
    let words: Vec<String> = args
        .iter()
        .map(|arg| shlex::try_quote(arg).map_or_else(|_| format!("{arg:?}"), |quoted| quoted.into_owned()))
        .collect();
    words.join(" ")
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

pub fn version(json: bool) -> i32 {
    let version = env!("CARGO_PKG_VERSION");
    if json {
        println!("{}", serde_json::json!({ "version": version }));
    } else {
        println!("agent-terminal {version}");
    }
    0
}
