use std::fmt;

use super::batch::batch_text;
use super::count;
use super::delivery::delivery;
use super::facts::fact_text;
use super::records::{command_text, history_text};
use super::screen::{screen, write_diff, write_refs};
use crate::protocol::{
    Checked, Closed, DashboardStopped, Delivered, Export, Matches, Owner, Reply, Screenshot, SessionInfo, Sessions,
    Squeeze, TabInfo, WaitOutcome,
};

pub(crate) fn reply_text(reply: &Reply, squeeze: &Squeeze) -> String {
    match reply {
        Reply::Created(created) => format!("created session {}\n", created.session),
        Reply::Snapshot(snapshot) => screen(snapshot, squeeze),
        Reply::Closed(closed) => closed_text(closed),
        Reply::Sessions(sessions) => sessions_text(sessions),
        Reply::Tabs(session) => tabs_text(session),
        Reply::Delivered(delivered) => delivered_text(delivered, squeeze),
        Reply::Waited(waited) => waited_text(waited),
        Reply::Checked(Checked { session, tab, fact }) => format!("{fact} in {session} {tab}\n"),
        Reply::Stopped | Reply::KeptRunning(_) => String::new(),
        Reply::Batch(outcomes) => batch_text(outcomes, squeeze),
        Reply::Fact(fact) => fact_text(fact),
        Reply::Matches(matches) => matches_text(matches),
        Reply::Marked(marked) => format!("marked {} at frame {}\n", marked.mark, marked.frame),
        Reply::Printed(printed) => printed.output.clone(),
        Reply::Diff(diff) => fmt::from_fn(|f| write_diff(f, diff)).to_string(),
        Reply::Exported(export) => exported_text(export),
        Reply::Screenshot(screenshot) => screenshot_text(screenshot),
        Reply::ScreenshotDiff(diff) => format!("{}\n", diff.summary()),
        Reply::Dashboard(dashboard) => format!("dashboard running at {}\n", dashboard.url),
        Reply::DashboardStopped(stopped) => dashboard_stopped_text(stopped),
        Reply::History(history) => history_text(history),
        Reply::Command(ran) => command_text(ran),
        Reply::Attached(attached) => format!("attached to {} {}\n", attached.session, attached.tab),
    }
}

fn closed_text(closed: &Closed) -> String {
    fmt::from_fn(|f| {
        closed.closed.iter().try_for_each(|tab| writeln!(f, "closed {} {}", tab.session, tab.tab))?;
        closed.ended.iter().try_for_each(|session| writeln!(f, "ended session {session}"))
    })
    .to_string()
}

fn sessions_text(sessions: &Sessions) -> String {
    if sessions.sessions.is_empty() {
        return "no sessions are open\n".into();
    }
    fmt::from_fn(|f| {
        sessions.sessions.iter().try_for_each(|session| {
            if session.tabs.is_empty() {
                writeln!(f, "{} · no tabs", session.session)
            } else {
                write_tab_lines(f, session)
            }
        })
    })
    .to_string()
}

fn tabs_text(session: &SessionInfo) -> String {
    if session.tabs.is_empty() {
        return format!("no tabs are open in session {}\n", session.session);
    }
    fmt::from_fn(|f| write_tab_lines(f, session)).to_string()
}

fn delivered_text(delivered: &Delivered, squeeze: &Squeeze) -> String {
    let shot = delivered.screenshot.as_deref().map(screenshot_text).unwrap_or_default();
    let shown = delivered.snapshot.as_deref().map(|snapshot| screen(snapshot, squeeze)).unwrap_or_default();
    format!("{}\n{shot}{shown}", delivery(delivered))
}

fn waited_text(waited: &WaitOutcome) -> String {
    let WaitOutcome { session, tab, until, waited_ms, exit_code, .. } = waited;
    let code = exit_code.map_or_else(String::new, |code| format!(" (exit code {code})"));
    format!("waited {waited_ms} ms for {until} in {session} {tab}{code}\n")
}

fn matches_text(matches: &Matches) -> String {
    fmt::from_fn(|f| {
        matches.matches.iter().try_for_each(|found| writeln!(f, "{},{} {}", found.row, found.col, found.text))
    })
    .to_string()
}

fn exported_text(export: &Export) -> String {
    let saved = format!(
        "exported session {} to {} ({}, {} KB)\n",
        export.session,
        export.path.display(),
        count(export.files, "file"),
        export.bytes.div_ceil(1024)
    );
    if export.missing.is_empty() {
        return saved;
    }
    let is = if export.missing.len() == 1 { "is" } else { "are" };
    let paths: Vec<String> = export.missing.iter().map(|path| path.display().to_string()).collect();
    format!(
        "{saved}{} the session saved {is} gone and {is} not in it: {}\n",
        count(export.missing.len(), "file"),
        paths.join(", ")
    )
}

fn dashboard_stopped_text(stopped: &DashboardStopped) -> String {
    match stopped {
        DashboardStopped { stopped: true } => "dashboard stopped\n".into(),
        DashboardStopped { stopped: false } => "no dashboard is running\n".into(),
    }
}

fn screenshot_text(screenshot: &Screenshot) -> String {
    let saved = format!(
        "saved {} ({} {}, frame {})\n",
        screenshot.path.display(),
        screenshot.session,
        screenshot.tab,
        screenshot.frame
    );
    match &screenshot.refs {
        Some(refs) => fmt::from_fn(|f| {
            f.write_str(&saved)?;
            write_refs(f, refs)
        })
        .to_string(),
        None => saved,
    }
}

fn write_tab_lines(f: &mut fmt::Formatter<'_>, session: &SessionInfo) -> fmt::Result {
    session.tabs.iter().try_for_each(|tab| writeln!(f, "{}", tab_line(session, tab).join(" · ")))
}

fn tab_line(session: &SessionInfo, tab: &TabInfo) -> Vec<String> {
    let running = &tab.terminal;
    let mut line = vec![
        format!("{} {}", session.session, tab.tab),
        running.program.clone(),
        format!("{}x{}", running.cols, running.rows),
    ];
    line.push(match (running.exited, running.exit_code) {
        (true, Some(code)) => format!("exited {code}"),
        (true, None) => "exited".into(),
        (false, _) => "running".into(),
    });
    if tab.control == Owner::Human {
        line.push("human control".into());
    }
    if session.current.as_ref() == Some(&tab.tab) {
        line.push("current".into());
    }
    line
}
