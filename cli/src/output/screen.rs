use std::fmt;

use super::count;
use crate::protocol::{DiffBase, Ref, RefKind, Snapshot, SnapshotDiff};

pub(crate) fn screen(s: &Snapshot) -> String {
    fmt::from_fn(|f| write_screen(f, s)).to_string()
}

fn write_screen(f: &mut fmt::Formatter<'_>, s: &Snapshot) -> fmt::Result {
    writeln!(f, "{}", header(s).join(" · "))?;
    let scrollback = s.scrollback.as_deref().unwrap_or_default();
    let width = label_width(s, scrollback);
    for (i, line) in scrollback.iter().enumerate() {
        let label = format!("-{}", scrollback.len() - i);
        writeln!(f, "{label:>width$}│{line}")?;
    }
    write_rows(f, s, width)?;
    match &s.refs {
        Some(refs) => write_refs(f, refs),
        None => Ok(()),
    }
}

fn header(s: &Snapshot) -> Vec<String> {
    let mut header = vec![format!("{} {}", s.session, s.tab), s.program.clone(), format!("{}x{}", s.cols, s.rows)];
    if s.alt_screen {
        header.push("alt-screen".into());
    }
    if let Some(mouse) = &s.mouse {
        header.push(format!("mouse:{mouse}"));
    }
    let [row, col] = s.cursor;
    header.push(if s.cursor_visible { format!("cursor {row},{col}") } else { "cursor hidden".into() });
    header.push(format!("frame {}", s.frame));
    match (s.exited, s.exit_code) {
        (true, Some(code)) => header.push(format!("exited {code}")),
        (true, None) => header.push("exited".into()),
        (false, _) => {}
    }
    header
}

fn label_width(s: &Snapshot, scrollback: &[String]) -> usize {
    let scrollback_width = if scrollback.is_empty() { 0 } else { scrollback.len().to_string().len() + 1 };
    s.rows.to_string().len().max(scrollback_width)
}

fn write_rows(f: &mut fmt::Formatter<'_>, s: &Snapshot, width: usize) -> fmt::Result {
    let first = usize::from(s.first_row);
    let shown = s.lines.iter().rposition(|line| !line.is_empty()).map_or(0, |i| i + 1);
    for (i, line) in s.lines.iter().take(shown).enumerate() {
        writeln!(f, "{:>width$}│{line}", first + i)?;
    }
    if shown == s.lines.len() {
        return Ok(());
    }
    let last = first + s.lines.len() - 1;
    let blank = if first + shown == last {
        format!("row {last} blank")
    } else {
        format!("rows {}-{last} blank", first + shown)
    };
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
