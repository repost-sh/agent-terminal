use crate::native::terminal::Terminal;
use crate::protocol::{ErrorBody, TabId};

pub(super) fn stopping() -> ErrorBody {
    ErrorBody::failure("the daemon is stopping because it was idle; run the command again to start a new one")
}

pub(super) fn no_terminal(session: &str) -> ErrorBody {
    ErrorBody::failure(format!(
        "no terminal is open in session {session}; start one with `agent-terminal open <program>`"
    ))
}

pub(super) fn no_tab(session: &str, id: TabId) -> ErrorBody {
    ErrorBody::failure(format!("no tab {id} in session {session}; list tabs with `agent-terminal tab`"))
}

pub(super) fn no_integration(session: &str, tab: &str, terminal: &Terminal) -> ErrorBody {
    ErrorBody::failure(format!(
        "{session} {tab} runs {} without shell integration, so it keeps no command history; exec, history and wait --command work in a tab opened with `agent-terminal shell`",
        terminal.program()
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
