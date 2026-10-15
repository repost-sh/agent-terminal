use super::Daemon;
use super::reading::trace_export;
use super::refusals::attach_on_its_own;
use crate::protocol::{DashboardStopped, ErrorBody, Reply, Request};

impl Daemon {
    pub(super) fn handle(&self, request: Request, typed: Option<&[String]>) -> Result<Reply, ErrorBody> {
        match request {
            Request::Open(open) => self.open(open),
            Request::Snapshot(snapshot) => self.snapshot(&snapshot),
            Request::Close(close) => self.close_request(&close),
            Request::Resize(resize) => self.resize(&resize),
            Request::Restart(restart) => self.restart(&restart),
            Request::Sessions => Ok(Reply::Sessions(self.list())),
            Request::Tabs(tabs) => Ok(Reply::Tabs(self.tabs_of(&tabs.session))),
            Request::WaitControl(waiting) => self.wait_control(waiting),
            Request::Attach(_) => Err(attach_on_its_own()),
            Request::SwitchTab(switch) => self.switch_tab(&switch),
            Request::Input(input) => self.input(input, typed),
            Request::Screenshot(screenshot) => self.screenshot_request(&screenshot, typed),
            Request::DiffScreenshot(diff) => self.diff_screenshot(diff),
            Request::Wait(wait) => self.wait(wait),
            Request::Expect(expect) => self.expect(&expect),
            Request::Get(get) => self.get(&get),
            Request::Find(find) => self.find(&find),
            Request::Mark(mark) => self.mark(mark),
            Request::Output(output) => self.output(&output),
            Request::DiffSnapshot(diff) => self.diff_snapshot(diff),
            Request::TraceExport(export) => trace_export(export),
            Request::Shell(shell) => self.open_shell(shell),
            Request::Exec(exec) => self.exec(exec),
            Request::History(history) => self.history(history),
            Request::HistoryShow(show) => self.history_show(show),
            Request::Stop => Ok(self.stop()),
            Request::StopIfNoTerminals => Ok(self.stop_if_no_terminals()),
            Request::Batch(batch) => Ok(self.batch(batch)),
            Request::DashboardStart(start) => self.start_dashboard(start).map(Reply::Dashboard),
            Request::DashboardStop => Ok(Reply::DashboardStopped(DashboardStopped { stopped: self.stop_dashboard() })),
        }
    }
}
