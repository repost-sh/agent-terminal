use std::fmt;

use super::count;
use crate::protocol::{DiffBase, Ref, RefKind, Snapshot, SnapshotDiff, Squeeze, Switch};

const SHORTEST_SQUEEZED_RUN: usize = 3;

pub(crate) fn screen(s: &Snapshot, squeeze: &Squeeze) -> String {
    fmt::from_fn(|f| write_screen(f, s, squeeze)).to_string()
}

fn write_screen(f: &mut fmt::Formatter<'_>, s: &Snapshot, squeeze: &Squeeze) -> fmt::Result {
    writeln!(f, "{}", header(s).join(" · "))?;
    let scrollback = s.scrollback.as_deref().unwrap_or_default();
    let width = label_width(s, scrollback);
    for (i, line) in scrollback.iter().enumerate() {
        let label = format!("-{}", scrollback.len() - i);
        writeln!(f, "{label:>width$}│{line}")?;
    }
    write_rows(f, s, width, squeeze)?;
    match &s.refs {
        Some(refs) => write_refs(f, refs),
        None => Ok(()),
    }
}

fn header(s: &Snapshot) -> Vec<String> {
    let [row, col] = s.cursor;
    let cursor = if s.cursor_visible { format!("cursor {row},{col}") } else { "cursor hidden".into() };
    let exit = match (s.exited, s.exit_code) {
        (true, Some(code)) => Some(format!("exited {code}")),
        (true, None) => Some("exited".into()),
        (false, _) => None,
    };
    [
        Some(format!("{} {}", s.session, s.tab)),
        Some(s.program.clone()),
        Some(format!("{}x{}", s.cols, s.rows)),
        (s.alt_screen == Switch::On).then(|| "alt-screen".into()),
        s.mouse.as_ref().map(|mouse| format!("mouse:{mouse}")),
        Some(cursor),
        Some(format!("frame {}", s.frame)),
        exit,
        s.nothing_printed.then(|| "nothing printed yet".into()),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn label_width(s: &Snapshot, scrollback: &[String]) -> usize {
    let scrollback_width = if scrollback.is_empty() { 0 } else { scrollback.len().to_string().len() + 1 };
    s.rows.to_string().len().max(scrollback_width)
}

fn write_rows(f: &mut fmt::Formatter<'_>, s: &Snapshot, width: usize, squeeze: &Squeeze) -> fmt::Result {
    let shown = s.lines.iter().rposition(|line| !line.is_empty()).map_or(0, |i| i + 1);
    let (printed, trailing) = s.lines.split_at(shown);
    let mut row = usize::from(s.first_row);
    for run in printed.chunk_by(|above, below| above.is_empty() == below.is_empty()) {
        if *squeeze == Squeeze::On && is_squeezed(run) {
            write_blank(f, width, row, run.len())?;
        } else {
            write_numbered(f, width, row, run)?;
        }
        row += run.len();
    }
    if trailing.is_empty() {
        return Ok(());
    }
    write_blank(f, width, row, trailing.len())
}

fn is_squeezed(run: &[String]) -> bool {
    run.len() >= SHORTEST_SQUEEZED_RUN && run.first().is_some_and(String::is_empty)
}

fn write_numbered(f: &mut fmt::Formatter<'_>, width: usize, first: usize, lines: &[String]) -> fmt::Result {
    lines.iter().enumerate().try_for_each(|(i, line)| writeln!(f, "{:>width$}│{line}", first + i))
}

fn write_blank(f: &mut fmt::Formatter<'_>, width: usize, first: usize, count: usize) -> fmt::Result {
    let last = first + count - 1;
    let blank = if count == 1 { format!("row {last} blank") } else { format!("rows {first}-{last} blank") };
    writeln!(f, "{:>width$}│ ({blank})", "…")
}

pub(super) fn write_refs(f: &mut fmt::Formatter<'_>, refs: &[Ref]) -> fmt::Result {
    if refs.is_empty() {
        return f.write_str("refs: none\n");
    }
    let kinds: Vec<String> = refs.iter().map(|r| format!("[{}]", r.kind)).collect();
    let labels: Vec<String> = refs.iter().map(|r| format!("\"{}\"", r.label)).collect();
    let widest = |texts: &mut dyn Iterator<Item = usize>| texts.max().unwrap_or(0);
    let id_width = widest(&mut refs.iter().map(|r| r.id.chars().count()));
    let kind_width = widest(&mut kinds.iter().map(|kind| kind.chars().count()));
    let label_width = widest(&mut labels.iter().map(|label| label.chars().count()));
    f.write_str("refs:\n")?;
    for ((r, kind), label) in refs.iter().zip(&kinds).zip(&labels) {
        let place = ref_place(r);
        writeln!(f, "  {:<id_width$} {kind:<kind_width$} {label:<label_width$} {place}", r.id)?;
    }
    Ok(())
}

fn ref_place(r: &Ref) -> String {
    let [top, bottom] = r.rows;
    match r.kind {
        RefKind::Panel => format!("rows {top}-{bottom}"),
        _ => format!("row {} col {}", r.row, r.col),
    }
}

pub(super) fn write_diff(f: &mut fmt::Formatter<'_>, diff: &SnapshotDiff) -> fmt::Result {
    let from = match &diff.since {
        DiffBase::Baseline(baseline) => baseline.clone(),
        DiffBase::Frame(frame) => format!("frame {frame}"),
    };
    let changed = match diff.changes.len() {
        0 => "no rows changed".to_string(),
        n => format!("{} changed", count(n, "row")),
    };
    writeln!(f, "{from} → frame {} · {changed}", diff.frame)?;
    let width = diff.changes.iter().map(|change| change.row.to_string().len()).max().unwrap_or(1);
    for change in &diff.changes {
        writeln!(f, "-{:>width$}│{}\n+{:>width$}│{}", change.row, change.before, change.row, change.after)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
