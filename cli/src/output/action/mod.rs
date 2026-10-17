use super::count;
use super::delivery::{Press, mouse_event, noted};
use super::waits::describe_wait;
use crate::key::Key;
use crate::protocol::request::{Close, Open, OpenShell};
use crate::protocol::{ClickTarget, Expect, Get, Input, Request, TextMatch};

pub(crate) fn action(request: &Request) -> Option<String> {
    Some(match request {
        Request::Input(deliver) => input_action(&deliver.input),
        Request::Wait(waited) => format!("waited for {}", describe_wait(&waited.wait)),
        Request::WaitControl(_) => "waited for control back".into(),
        Request::Expect(expected) => expect_action(&expected.expect),
        Request::Get(get) => format!("read the {}", fact_name(get.what)),
        Request::Find(find) => {
            format!("looked for {}", TextMatch { text: find.text.clone(), regex: find.regex, rows: None })
        }
        Request::Mark(mark) => format!("marked \"{}\"", mark.name),
        Request::Exec(exec) => format!("ran \"{}\"", exec.line),
        Request::Open(Open { tab, .. }) | Request::Shell(OpenShell { tab, .. }) => format!("opened {tab}"),
        Request::Resize(resize) => format!("resized to {}x{}", resize.size.cols, resize.size.rows),
        Request::Restart(_) => "restarted".into(),
        Request::Close(Close::Tab(_)) => "closed the tab".into(),
        Request::Close(Close::Session { .. }) => "closed the session".into(),
        Request::SwitchTab(switch) => format!("switched to {}", switch.tab),
        Request::RenameTab(rename) => format!("renamed the tab to {}", rename.name),
        Request::Tabs(_) => "listed the tabs".into(),
        Request::Snapshot(_) => "read the screen".into(),
        Request::Screenshot(_) => "took a screenshot".into(),
        Request::DiffSnapshot(_) => "compared the screen with its baseline".into(),
        Request::DiffScreenshot(_) => "compared a screenshot with its baseline".into(),
        Request::Output(_) => "read the output".into(),
        Request::History(_) => "read the command history".into(),
        Request::HistoryShow(show) => format!("read command {}", show.id),
        Request::TraceExport(_) => "exported a trace".into(),
        Request::NewSession(_)
        | Request::Sessions
        | Request::Close(Close::EverySession)
        | Request::Batch(_)
        | Request::Stop
        | Request::StopIfNoTerminals
        | Request::DashboardStart(_)
        | Request::DashboardStop
        | Request::Attach(_) => return None,
    })
}

pub(crate) fn input_action(input: &Input) -> String {
    match input {
        Input::Type { text, .. } => format!("typed \"{text}\""),
        Input::Press { keys, repeat } => pressed(keys, *repeat),
        Input::Paste { text } => format!("pasted \"{text}\""),
        Input::Send { bytes } => format!("sent {}", count(bytes.len(), "raw byte")),
        Input::Signal { signal } => format!("sent {signal}"),
        Input::Click { target, button, double, mods } => {
            let what = match target {
                ClickTarget::Text(text) => format!("\"{text}\""),
                other => other.to_string(),
            };
            format!("clicked {what}{}", noted(&Press { button: *button, double: *double, mods: *mods }.notes()))
        }
        Input::Mouse { event } => {
            let (what, notes) = mouse_event(*event);
            format!("{what}{}", noted(&notes))
        }
        Input::Scroll { direction, count: steps, .. } => format!("scrolled {direction} {steps}"),
        Input::Focus { focused } => format!("reported focus {}", if *focused { "in" } else { "out" }),
    }
}

fn pressed(keys: &[Key], repeat: u32) -> String {
    let keys = keys.iter().map(ToString::to_string).collect::<Vec<_>>().join(" ");
    if repeat > 1 { format!("pressed {keys} {repeat} times") } else { format!("pressed {keys}") }
}

fn expect_action(expect: &Expect) -> String {
    match expect {
        Expect::Text(text) => format!("expected {text}"),
        Expect::Gone(text) => format!("expected {text} to be gone"),
        Expect::ExitCode { code } => format!("expected exit code {code}"),
        Expect::Cell { row, col, .. } => format!("expected the style at {row},{col}"),
    }
}

fn fact_name(get: Get) -> String {
    match get {
        Get::Text => "text".into(),
        Get::Cursor => "cursor".into(),
        Get::Size => "size".into(),
        Get::Frame => "frame".into(),
        Get::Program => "program".into(),
        Get::ExitCode => "exit code".into(),
        Get::Pid => "pid".into(),
        Get::Title => "title".into(),
        Get::Cwd => "working directory".into(),
        Get::Modes => "modes".into(),
        Get::Cell { row, col } => format!("cell at {row},{col}"),
    }
}

#[cfg(test)]
mod tests;
