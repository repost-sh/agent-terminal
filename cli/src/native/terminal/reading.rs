use std::sync::PoisonError;

use super::state::{Listed, Shown, State};
use super::{Labelled, Terminal};
use crate::native::output_log::Output;
use crate::native::refs::{self, Found, Grid};
use crate::native::wait;
use crate::protocol::{
    Baseline, Cell, Cursor, DiffBase, Exit, Fact, Get, Match, Ref, RowChange, RowRange, Scrollback, Size, Snapshot,
    SnapshotDiff, SnapshotOptions, request,
};

pub(crate) struct Capture {
    pub grid: Grid,
    pub rows: RowRange,
    pub frame: u64,
    pub refs: Option<Vec<Ref>>,
}

struct Rendered<'a> {
    session: &'a str,
    tab: &'a str,
    range: RowRange,
}

impl Labelled<'_> {
    pub(crate) fn snapshot(&self) -> Snapshot {
        let mut state = self.terminal.state.lock().unwrap_or_else(PoisonError::into_inner);
        let rows = state.screen.view().size.rows;
        let range = RowRange { first: 1, last: rows };
        let rendered = Rendered { session: self.session, tab: self.tab, range };
        self.terminal.render(&mut state, &rendered, &SnapshotOptions::default(), None)
    }

    pub(crate) fn snapshot_with(&self, options: &SnapshotOptions) -> Result<Snapshot, String> {
        let mut state = self.terminal.state.lock().unwrap_or_else(PoisonError::into_inner);
        let range = row_range(state.screen.size(), options.rows)?;
        let refs = options.refs.then(|| list_refs(&mut state)).transpose()?;
        Ok(self.terminal.render(&mut state, &Rendered { session: self.session, tab: self.tab, range }, options, refs))
    }
}

impl Terminal {
    pub(crate) fn capture(&self, rows: Option<RowRange>, refs: bool) -> Result<Capture, String> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let rows = row_range(state.screen.size(), rows)?;
        let grid = state.screen.grid();
        let refs = refs.then(|| remember_refs(&mut state, &grid)).transpose()?;
        Ok(Capture { grid, rows, frame: state.frame, refs })
    }

    pub(crate) fn text_rows(&self, rows: Option<RowRange>, ansi: bool) -> Result<(u64, Vec<String>), String> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let rows = row_range(state.screen.size(), rows)?;
        Ok((state.frame, state.screen.rows(i32::from(rows.first) - 1..i32::from(rows.last), ansi)))
    }

    fn render(
        &self,
        state: &mut State,
        rendered: &Rendered<'_>,
        options: &SnapshotOptions,
        refs: Option<Vec<Ref>>,
    ) -> Snapshot {
        let view = state.screen.view();
        state.last_shown = Some(Shown { frame: state.frame, lines: view.lines.clone() });
        let history = state.screen.scrollback_len();
        let scrollback = match options.scrollback {
            Scrollback::None => None,
            Scrollback::All => Some(history),
            Scrollback::Last(n) => Some(n.min(history)),
        };
        let range = rendered.range;
        Snapshot {
            session: rendered.session.to_string(),
            tab: rendered.tab.to_string(),
            program: self.program.clone(),
            cols: view.size.cols,
            rows: view.size.rows,
            alt_screen: view.alt_screen,
            mouse: view.mouse.map(|m| m.as_str().to_string()),
            cursor: [view.cursor.0 + 1, view.cursor.1 + 1],
            cursor_visible: view.cursor_visible,
            frame: state.frame,
            exited: state.status.exited(),
            exit_code: state.status.code(),
            first_row: range.first,
            scrollback: scrollback.map(|n| state.screen.rows(-lines(n)..0, options.ansi)),
            lines: state.screen.rows(i32::from(range.first) - 1..i32::from(range.last), options.ansi),
            refs,
        }
    }

    pub(crate) fn fact(&self, what: Get) -> Result<Fact, String> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let view = state.screen.view();
        Ok(match what {
            Get::Text => {
                let shown = view.lines.iter().rposition(|line| !line.is_empty()).map_or(0, |i| i + 1);
                Fact::Text(view.lines.iter().take(shown).cloned().collect::<Vec<_>>().join("\n"))
            }
            Get::Cursor => {
                Fact::Cursor(Cursor { row: view.cursor.0 + 1, col: view.cursor.1 + 1, visible: view.cursor_visible })
            }
            Get::Size => Fact::Size(view.size),
            Get::Frame => Fact::Frame(state.frame),
            Get::Program => Fact::Program(self.program.clone()),
            Get::ExitCode => Fact::ExitCode(Exit { exited: state.status.exited(), code: state.status.code() }),
            Get::Pid => Fact::Pid(self.pid),
            Get::Title => Fact::Title(state.screen.title()),
            Get::Cwd => Fact::Cwd(state.screen.reported_cwd().unwrap_or_else(|| self.cwd.clone())),
            Get::Modes => Fact::Modes(state.screen.modes()),
            Get::Cell { row, col } => Fact::Cell(state.screen.cell_at(row, col)?),
        })
    }

    pub(crate) fn find(&self, find: &request::Find) -> Result<Vec<Match>, String> {
        let pattern = wait::compile(&find.text, find.regex)?;
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let first = if find.all { -lines(state.screen.scrollback_len()) } else { 0 };
        let matches = state.screen.matches(&pattern, first);
        if matches.is_empty() {
            let place = if find.all { "on the screen or in its scrollback" } else { "on the screen" };
            return Err(format!("{:?} is not {place}", find.text));
        }
        Ok(matches)
    }

    pub(crate) fn diff_snapshot(&self, baseline: Option<Baseline>) -> Result<SnapshotDiff, String> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let current = Shown { frame: state.frame, lines: state.screen.view().lines };
        let (since, before) = if let Some(baseline) = baseline {
            (DiffBase::Baseline(baseline.name), baseline.lines)
        } else {
            let shown = state
                .last_shown
                .take()
                .ok_or("nothing to compare with; take a snapshot first with `agent-terminal snapshot`".to_string())?;
            (DiffBase::Frame(shown.frame), shown.lines)
        };
        let changes = changed_rows(&before, &current.lines)?;
        let frame = current.frame;
        if let DiffBase::Frame(_) = since {
            state.last_shown = Some(current);
        }
        Ok(SnapshotDiff { since, frame, changes })
    }

    pub(crate) fn mark(&self, name: String) -> u64 {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let frame = state.frame;
        state.log.mark(name, frame);
        frame
    }

    pub(crate) fn output_since(&self, raw: u64, styled: u64) -> Output {
        self.state.lock().unwrap_or_else(PoisonError::into_inner).log.output_since(raw, styled)
    }

    pub(crate) fn printed(&self, asked: &request::Output) -> Result<String, String> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.log.since(asked, state.frame)
    }

    pub(crate) fn lines(&self) -> Vec<String> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner).screen.view().lines
    }

    pub(crate) fn cell(&self, row: u16, col: u16) -> Result<Cell, String> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner).screen.cell_at(row, col)
    }
}

fn lines(count: usize) -> i32 {
    i32::try_from(count).unwrap_or(i32::MAX)
}

fn changed_rows(before: &[String], after: &[String]) -> Result<Vec<RowChange>, String> {
    let row = |lines: &[String], i: usize| lines.get(i).map_or("", String::as_str).to_string();
    (0..before.len().max(after.len()))
        .map(|i| (i, row(before, i), row(after, i)))
        .filter(|(_, before, after)| before != after)
        .map(|(i, before, after)| {
            let row = u16::try_from(i + 1).map_err(|e| format!("row {} cannot be compared: {e}", i + 1))?;
            Ok(RowChange { row, before, after })
        })
        .collect()
}

fn row_range(size: Size, rows: Option<RowRange>) -> Result<RowRange, String> {
    match rows {
        None => Ok(RowRange { first: 1, last: size.rows }),
        Some(RowRange { first, last }) if first == 0 || first > size.rows || first > last => {
            Err(format!("rows {first}:{last} are outside the {}x{} screen", size.cols, size.rows))
        }
        Some(RowRange { first, last }) => Ok(RowRange { first, last: last.min(size.rows) }),
    }
}

fn list_refs(state: &mut State) -> Result<Vec<Ref>, String> {
    let grid = state.screen.grid();
    remember_refs(state, &grid)
}

fn remember_refs(state: &mut State, grid: &Grid) -> Result<Vec<Ref>, String> {
    let found = refs::find(grid)?;
    let frame = state.frame;
    let listed = found.iter().enumerate().map(|(index, found)| listed_ref(index, found, frame)).collect();
    state.listed = Some(Listed { frame, refs: found });
    Ok(listed)
}

fn listed_ref(index: usize, found: &Found, frame: u64) -> Ref {
    let place = |at: usize| u16::try_from(at + 1).unwrap_or(u16::MAX);
    let area = found.area();
    Ref {
        id: format!("@t{}", index + 1),
        kind: found.kind(),
        label: found.label().to_string(),
        row: place(found.row()),
        col: place(found.col()),
        rows: [place(area.row), place(area.last_row)],
        cols: [place(area.col), place(area.last_col)],
        frame,
        url: found.url().map(str::to_string),
    }
}
