use crate::protocol::{DiffBase, ErrorBody, Exit, Fact, Modes, Reply, Snapshot, SnapshotDiff};

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
        Reply::Stopped => {}
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
    }
}

fn diff_text(diff: &SnapshotDiff) -> String {
    let from = match &diff.since {
        DiffBase::Baseline(baseline) => baseline.clone(),
        DiffBase::Frame(frame) => format!("frame {frame}"),
    };
    let changed = match diff.changes.len() {
        0 => "no rows changed".to_string(),
        1 => "1 row changed".to_string(),
        n => format!("{n} rows changed"),
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
        Fact::Cell(cell) => {
            let mut text = format!("{:?} fg={} bg={}", cell.char, cell.fg, cell.bg);
            for (on, name) in [(cell.bold, "bold"), (cell.inverse, "inverse"), (cell.underline, "underline")] {
                if on {
                    text.push(' ');
                    text.push_str(name);
                }
            }
            text.push('\n');
            text
        }
    }
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
