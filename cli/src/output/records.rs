use std::fmt;

use crate::protocol::{CommandOutput, CommandRecord, History};

pub(super) fn history_text(history: &History) -> String {
    if history.total == 0 {
        return format!("no commands have run in {} {}\n", history.session, history.tab);
    }
    if history.commands.is_empty() {
        return format!("no command matches in {} {} ({} in all)\n", history.session, history.tab, history.total);
    }
    fmt::from_fn(|f| history.commands.iter().try_for_each(|record| writeln!(f, "{}", record_line(record)))).to_string()
}

pub(super) fn command_text(ran: &CommandOutput) -> String {
    let gap = if !ran.output.is_empty() && !ran.output.ends_with('\n') { "\n" } else { "" };
    format!("{}{gap}{}\n", ran.output, record_line(&ran.record))
}

fn record_line(record: &CommandRecord) -> String {
    let status = match (record.finished(), record.exit_code) {
        (false, _) => "running".to_string(),
        (true, Some(code)) => format!("exit {code}"),
        (true, None) => "exit unknown".to_string(),
    };
    format!("{} · {status} · {} · {} · {}", record.id, duration(record.duration_ms), record.cwd, record.command)
}

fn duration(ms: u64) -> String {
    if ms < 1_000 {
        return format!("{ms} ms");
    }
    match u32::try_from(ms) {
        Ok(under_a_minute) if ms < 60_000 => format!("{:.1} s", f64::from(under_a_minute) / 1000.0),
        _ => format!("{} min {:02} s", ms / 60_000, ms % 60_000 / 1000),
    }
}
