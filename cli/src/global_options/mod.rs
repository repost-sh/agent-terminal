use std::path::PathBuf;

use clap::{ArgAction, Args};

use crate::protocol::Name;

pub(crate) const NAME_THE_SESSION: &str = "name this conversation's session after its task and create it first: `agent-terminal session new repost-forward`, then pass `--session repost-forward` on every call (or export AGENT_TERMINAL_SESSION)";

#[derive(Args)]
pub(crate) struct GlobalOptions {
    #[arg(
        long,
        global = true,
        help = "This conversation's session, made by `session new <name>` (or AGENT_TERMINAL_SESSION)",
        env = "AGENT_TERMINAL_SESSION",
        value_name = "NAME",
        value_parser = clap::value_parser!(Name)
    )]
    pub session: Option<Name>,

    #[arg(long, global = true, help = "JSON output")]
    pub json: bool,

    #[arg(long, global = true, help = "Give up waiting after this long", value_name = "MS", default_value_t = 25_000)]
    pub timeout: u64,

    #[arg(
        long,
        global = true,
        help = "Act on the tab with this name instead of the current one",
        value_name = "NAME",
        value_parser = clap::value_parser!(Name)
    )]
    pub tab: Option<Name>,

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

#[cfg(test)]
mod tests;
