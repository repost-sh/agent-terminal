use std::time::Duration;

use super::Session;
use crate::native::terminal::Labelled;
use crate::output;
use crate::protocol::{ErrorBody, Name};

pub(super) fn stopping() -> ErrorBody {
    ErrorBody::failure("the daemon is stopping; run the command again to start a new one")
}

pub(super) fn known_session<T>(session: &str, found: Option<T>) -> Result<T, ErrorBody> {
    found.ok_or_else(|| {
        ErrorBody::failure(format!("no session {session}; create it with `agent-terminal session new {session}`"))
    })
}

pub(super) fn taken(session: &Name, held: &Session) -> ErrorBody {
    let names: Vec<&str> = held.tabs.values().map(|tab| &*tab.name).collect();
    let tabs = match names.len() {
        0 => "no tabs".to_string(),
        k => format!("{}: {}", output::count(k, "tab"), names.join(", ")),
    };
    ErrorBody::failure(format!(
        "session {session} is taken: open for {}, {tabs}; pick another name for this conversation",
        age(held.created.elapsed())
    ))
}

pub(super) fn age(open_for: Duration) -> String {
    let seconds = open_for.as_secs();
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m", seconds / 60),
        _ => format!("{}h {}m", seconds / 3600, seconds % 3600 / 60),
    }
}

pub(super) fn no_terminal(session: &str) -> ErrorBody {
    ErrorBody::failure(format!(
        "no terminal is open in session {session}; start one with `agent-terminal open --name <name> <program>`"
    ))
}

pub(super) fn no_tab(session: &str, tab: &str) -> ErrorBody {
    ErrorBody::failure(format!("no tab {tab} in session {session}; list tabs with `agent-terminal tab`"))
}

pub(super) fn tab_taken(holder: &Labelled<'_>) -> ErrorBody {
    let Labelled { session, tab, terminal } = holder;
    ErrorBody::failure(format!("tab {tab} already runs {} in session {session}; pick another name", terminal.program()))
}

pub(super) fn no_integration(at: &Labelled<'_>) -> ErrorBody {
    ErrorBody::failure(format!(
        "{} {} runs {} without shell integration, so it keeps no command history; exec, history and wait --command work in a tab opened with `agent-terminal shell`",
        at.session,
        at.tab,
        at.terminal.program()
    ))
}

pub(super) fn human_has_control(session: &str, tab: &str) -> ErrorBody {
    ErrorBody::new(
        crate::exit::HUMAN_CONTROL,
        format!("A human has control of {session} {tab}. Run `agent-terminal wait --control`."),
    )
}

pub(super) fn attach_on_its_own() -> ErrorBody {
    ErrorBody::failure(
        "attach shows the tab in your terminal, so it runs on its own, not in batch or from the dashboard",
    )
}

pub(super) fn error_body(error: &anyhow::Error) -> ErrorBody {
    let code = error.downcast_ref::<crate::failure::Failure>().map_or(crate::exit::ERROR, |e| e.code);
    ErrorBody::new(code, format!("{error:#}"))
}
