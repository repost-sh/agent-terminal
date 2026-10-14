use crate::protocol::{Delivered, Delivery, ErrorBody, Reply, Snapshot};

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
        Reply::Delivered(delivered) if json => println!("{}", serde_json::json!(delivered)),
        Reply::Delivered(delivered) => println!("{}", delivery(delivered)),
        Reply::Stopped => {}
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
    }
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {noun}s") }
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
