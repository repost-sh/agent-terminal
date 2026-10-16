use crate::protocol::{Exit, Fact, Modes};

pub(super) fn fact_text(fact: &Fact) -> String {
    match fact {
        Fact::Text(text) if text.is_empty() => String::new(),
        Fact::Text(text) | Fact::Program(text) | Fact::Title(Some(text)) | Fact::Cwd(text) => format!("{text}\n"),
        Fact::Cursor(cursor) if cursor.visible => format!("{},{}\n", cursor.row, cursor.col),
        Fact::Cursor(_) => "hidden\n".into(),
        Fact::Size(size) => format!("{}x{}\n", size.cols, size.rows),
        Fact::Frame(frame) => format!("{frame}\n"),
        Fact::ExitCode(exit) => exit_text(exit),
        Fact::Pid(Some(pid)) => format!("{pid}\n"),
        Fact::Pid(None) => "unknown\n".into(),
        Fact::Title(None) => String::new(),
        Fact::Modes(modes) => modes_text(modes),
        Fact::Cell(cell) => format!("{cell}\n"),
    }
}

fn exit_text(exit: &Exit) -> String {
    match exit {
        Exit { exited: false, .. } => "running\n".into(),
        Exit { code: Some(code), .. } => format!("{code}\n"),
        Exit { code: None, .. } => "exited, exit code unknown\n".into(),
    }
}

fn modes_text(modes: &Modes) -> String {
    let mouse = modes.mouse.as_ref().map_or_else(|| "off".into(), |m| format!("{}, {}", m.tracking, m.encoding));
    let keyboard = if modes.keyboard.is_empty() { "legacy".into() } else { modes.keyboard.join(", ") };
    format!(
        "alt-screen: {}\nmouse: {mouse}\nbracketed-paste: {}\napp-cursor: {}\nfocus-events: {}\nkeyboard: {keyboard}\n",
        modes.alt_screen, modes.bracketed_paste, modes.app_cursor, modes.focus_events,
    )
}
