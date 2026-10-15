use super::Daemon;
use super::refusals::{error_body, no_integration};
use crate::connection::home;
use crate::native::screenshots;
use crate::native::store;
use crate::protocol::{
    CommandOutput, CommandRecord, ErrorBody, Export, History, Marked, Matches, Printed, Reply, request,
};

impl Daemon {
    pub(super) fn snapshot(&self, snapshot: &request::Snapshot) -> Result<Reply, ErrorBody> {
        let (id, terminal) = self.target(&snapshot.target)?;
        terminal
            .snapshot_with(&snapshot.target.session, &id.to_string(), &snapshot.options)
            .map(Reply::Snapshot)
            .map_err(ErrorBody::failure)
    }

    pub(super) fn get(&self, get: &request::Get) -> Result<Reply, ErrorBody> {
        let (_, terminal) = self.target(&get.target)?;
        terminal.fact(get.what).map(Reply::Fact).map_err(ErrorBody::failure)
    }

    pub(super) fn find(&self, find: &request::Find) -> Result<Reply, ErrorBody> {
        let (_, terminal) = self.target(&find.target)?;
        let matches = terminal.find(&find.text, find.regex, find.all).map_err(ErrorBody::failure)?;
        Ok(Reply::Matches(Matches { matches }))
    }

    pub(super) fn mark(&self, mark: request::Mark) -> Result<Reply, ErrorBody> {
        let (_, terminal) = self.target(&mark.target)?;
        let frame = terminal.mark(mark.name.clone());
        Ok(Reply::Marked(Marked { mark: mark.name, frame }))
    }

    pub(super) fn output(&self, output: &request::Output) -> Result<Reply, ErrorBody> {
        let (_, terminal) = self.target(&output.target)?;
        let output = terminal.printed(output.since.as_deref(), output.raw).map_err(ErrorBody::failure)?;
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
        let (id, terminal) = self.target(&screenshot.target)?;
        self.screenshot(&screenshot.target.session, &id.to_string(), &terminal, &screenshot.shot, typed)
            .map(Reply::Screenshot)
    }

    pub(super) fn diff_screenshot(&self, diff: request::DiffScreenshot) -> Result<Reply, ErrorBody> {
        let (id, terminal) = self.target(&diff.target)?;
        screenshots::diff(&diff.target.session, &id.to_string(), &terminal, diff.baseline, &diff.shot, diff.threshold)
            .map(Reply::ScreenshotDiff)
            .map_err(|e| error_body(&e))
    }

    pub(super) fn history(&self, history: request::History) -> Result<Reply, ErrorBody> {
        let (id, terminal) = self.target(&history.target)?;
        let (session, tab) = (history.target.session, id.to_string());
        let mut commands = terminal.commands().ok_or_else(|| no_integration(&session, &tab, &terminal))?;
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
        let (tab_id, terminal) = self.target(&show.target)?;
        let (session, tab, id) = (show.target.session, tab_id.to_string(), show.id);
        if terminal.shell().is_none() {
            return Err(no_integration(&session, &tab, &terminal));
        }
        let (record, output) = terminal.command(id).ok_or_else(|| {
            ErrorBody::failure(format!("no command {id} in {session} {tab}; list them with `agent-terminal history`"))
        })?;
        Ok(Reply::Command(CommandOutput { session, tab, record, output, check: false }))
    }
}

pub(super) fn trace_export(export: request::TraceExport) -> Result<Reply, ErrorBody> {
    let request::TraceExport { session, path } = export;
    let exported = home().and_then(|home| store::export(&home, &session, &path)).map_err(|e| error_body(&e))?;
    Ok(Reply::Exported(Export {
        session,
        path,
        files: exported.files,
        bytes: exported.bytes,
        missing: exported.missing,
    }))
}
