use std::path::PathBuf;
use std::time::Duration;

use clap::{ArgAction, Args};

use crate::protocol::TabId;

#[derive(Args)]
pub(crate) struct GlobalOptions {
    #[arg(
        long,
        global = true,
        help = "Use a named session (or AGENT_TERMINAL_SESSION)",
        env = "AGENT_TERMINAL_SESSION",
        default_value = "default"
    )]
    pub session: String,

    #[arg(long, global = true, help = "JSON output")]
    pub json: bool,

    #[arg(long, global = true, help = "Give up waiting after this long", value_name = "MS", default_value_t = 25_000)]
    pub timeout: u64,

    #[arg(long, global = true, help = "Act on this tab instead of the current one", value_name = "ID", value_parser = clap::value_parser!(TabId))]
    pub tab: Option<TabId>,

    #[arg(
        long,
        global = true,
        help = "Stop an unused daemon after this long: 30s, 10m, 1h or raw ms, 0 = never (set when a call starts the daemon)",
        value_name = "TIME",
        default_value = "1h",
        value_parser = parse_duration
    )]
    pub idle_timeout: Duration,

    #[arg(
        long,
        global = true,
        help = "Where screenshots go when the command names no file (default: ~/.agent-terminal/screenshots)",
        env = "AGENT_TERMINAL_SCREENSHOT_DIR",
        value_name = "PATH"
    )]
    pub screenshot_dir: Option<PathBuf>,
}

pub(crate) fn without_global_options(typed: &[String]) -> Vec<String> {
    let options = GlobalOptions::augment_args(clap::Command::new("agent-terminal"));
    let global = |word: &str| {
        let name = word.strip_prefix("--")?.split('=').next()?;
        let arg = options.get_arguments().find(|arg| arg.get_long() == Some(name))?;
        let takes_value = matches!(arg.get_action(), ArgAction::Set | ArgAction::Append);
        Some(takes_value && !word.contains('='))
    };
    let mut words = Vec::new();
    let mut rest = typed.iter();
    while let Some(word) = rest.next() {
        if word == "--" {
            words.push(word.clone());
            words.extend(rest.cloned());
            break;
        }
        match global(word) {
            Some(true) => {
                rest.next();
            }
            Some(false) => {}
            None => words.push(word.clone()),
        }
    }
    words
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    let refused = || format!("invalid time: {value} (expected a duration like 30s, 10m, 1h or raw ms (0 = never))");
    let unit_at = value.find(|c: char| !c.is_ascii_digit()).unwrap_or(value.len());
    let (number, unit) = value.split_at(unit_at);
    let number: u64 = number.parse().map_err(|e| format!("{}: {e}", refused()))?;
    let millis_per_unit = match unit {
        "" | "ms" => 1,
        "s" => 1_000,
        "m" => 60_000,
        "h" => 3_600_000,
        _ => return Err(refused()),
    };
    number.checked_mul(millis_per_unit).map(Duration::from_millis).ok_or_else(refused)
}
