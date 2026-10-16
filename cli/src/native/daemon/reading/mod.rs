use std::path::Path;

use super::Daemon;
use super::input::Shooting;
use super::refusals::{error_body, no_integration};
use crate::native::screenshots;
use crate::native::store;
use crate::native::terminal::Labelled;
use crate::protocol::{
    CommandOutput, CommandRecord, ErrorBody, Export, History, Marked, Matches, Printed, Reply, request,
};

impl Daemon {
    pub(super) fn snapshot(&self, snapshot: &request::Snapshot) -> Result<Reply, ErrorBody> {
        let (tab, terminal) = self.target(&snapshot.target)?;
        Labelled { session: &snapshot.target.session, tab: &tab, terminal: &terminal }
            .snapshot_with(&snapshot.options)
            .map(Reply::Snapshot)
            .map_err(ErrorBody::failure)
    }

    pub(super) fn get(&self, get: &request::Get) -> Result<Reply, ErrorBody> {
        let (_, terminal) = self.target(&get.target)?;
        terminal.fact(get.what).map(Reply::Fact).map_err(ErrorBody::failure)
    }

    pub(super) fn find(&self, find: &request::Find) -> Result<Reply, ErrorBody> {
        let (_, terminal) = self.target(&find.target)?;
        let matches = terminal.find(find).map_err(ErrorBody::failure)?;
        Ok(Reply::Matches(Matches { matches }))
    }

    pub(super) fn mark(&self, mark: request::Mark) -> Result<Reply, ErrorBody> {
        let (_, terminal) = self.target(&mark.target)?;
        let frame = terminal.mark(mark.name.clone());
        Ok(Reply::Marked(Marked { mark: mark.name, frame }))
    }

    pub(super) fn output(&self, output: &request::Output) -> Result<Reply, ErrorBody> {
        let (_, terminal) = self.target(&output.target)?;
        let output = terminal.printed(output).map_err(ErrorBody::failure)?;
        Ok(Reply::Printed(Printed { output }))
    }

    pub(super) fn diff_snapshot(&self, diff: request::DiffSnapshot) -> Result<Reply, ErrorBody> {
        let (_, terminal) = self.target(&diff.target)?;
        terminal.diff_snapshot(diff.baseline).map(Reply::Diff).map_err(ErrorBody::failure)
    }

    pub(super) fn screenshot_request(
        &self,
        screenshot: &request::Screenshot,
        typed: Option<&[String]>,
    ) -> Result<Reply, ErrorBody> {
        let (tab, terminal) = self.target(&screenshot.target)?;
        let at = Labelled { session: &screenshot.target.session, tab: &tab, terminal: &terminal };
        self.screenshot(&Shooting { at, shot: &screenshot.shot, typed }).map(Reply::Screenshot)
    }

    pub(super) fn diff_screenshot(&self, diff: request::DiffScreenshot) -> Result<Reply, ErrorBody> {
        let (tab, terminal) = self.target(&diff.target)?;
        let session = diff.target.session.clone();
        screenshots::diff(&Labelled { session: &session, tab: &tab, terminal: &terminal }, diff)
            .map(Reply::ScreenshotDiff)
            .map_err(|e| error_body(&e))
    }

    pub(super) fn history(&self, history: request::History) -> Result<Reply, ErrorBody> {
        let (tab, terminal) = self.target(&history.target)?;
        let (session, tab) = (String::from(history.target.session), tab.to_string());
        let mut commands = terminal
            .commands()
            .ok_or_else(|| no_integration(&Labelled { session: &session, tab: &tab, terminal: &terminal }))?;
        let total = commands.len();
        if history.failed {
            commands.retain(CommandRecord::failed);
        }
        if let Some(last) = history.last {
            commands.drain(..commands.len().saturating_sub(last));
        }
        Ok(Reply::History(History { session, tab, total, commands }))
    }

    pub(super) fn history_show(&self, show: request::HistoryShow) -> Result<Reply, ErrorBody> {
        let (tab, terminal) = self.target(&show.target)?;
        let (session, tab, id) = (String::from(show.target.session), tab.to_string(), show.id);
        if terminal.shell().is_none() {
            return Err(no_integration(&Labelled { session: &session, tab: &tab, terminal: &terminal }));
        }
        let (record, output) = terminal.command(id).ok_or_else(|| {
            ErrorBody::failure(format!("no command {id} in {session} {tab}; list them with `agent-terminal history`"))
        })?;
        Ok(Reply::Command(CommandOutput { session, tab, record, output, check: false }))
    }
}

pub(super) fn trace_export(home: &Path, export: request::TraceExport) -> Result<Reply, ErrorBody> {
    let exported = store::export(home, &export).map_err(|e| error_body(&e))?;
    let request::TraceExport { session, path } = export;
    Ok(Reply::Exported(Export {
        session: session.to_string(),
        path,
        files: exported.files,
        bytes: exported.bytes,
        missing: exported.missing,
    }))
}

#[cfg(test)]
mod tests;
