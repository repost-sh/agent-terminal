use crate::protocol::{BatchOutcome, Delivered, Delivery, ErrorBody, Reply, Response, Snapshot};

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
        Reply::Batch(outcomes) if json => println!("{}", batch_json(outcomes)),
        Reply::Batch(outcomes) => batch(outcomes),
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
