use std::fmt::{self, Write as _};

use anyhow::Result;

use super::theme::{self, Palette, Rgb};
use super::{Drawing, Entry, Framed, Layout, Placed, cell_colors, span_of};
use crate::native::refs::{Decoration, Glyph, Grid};
use crate::protocol::Ref;

const FONT_FAMILY: &str = "'JetBrains Mono', 'JetBrainsMono Nerd Font', ui-monospace, Menlo, Consolas, monospace";

pub(crate) struct Svg {
    pub text: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Font {
    bold: bool,
    italic: bool,
}

#[derive(Clone, PartialEq)]
struct Style {
    fg: Rgb,
    bg: Rgb,
    font: Font,
    lines: Decoration,
}

impl Style {
    fn decorations(&self) -> Vec<&'static str> {
        [(self.lines.underline, "underline"), (self.lines.strikeout, "line-through")]
            .into_iter()
            .filter_map(|(on, name)| on.then_some(name))
            .collect()
    }
}

struct Run {
    col: u16,
    cells: u16,
    text: String,
    style: Style,
}

fn runs(cells: &[Glyph], palette: &Palette, cursor_at: Option<usize>) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    let mut col: u16 = 0;
    while let Some(glyph) = cells.get(usize::from(col)) {
        let span = span_of(glyph, col, cells.len());
        let style = style(glyph, palette, cursor_at == Some(usize::from(col)));
        let text = if glyph.text.is_empty() { " " } else { glyph.text.as_str() };
        extend(&mut runs, Run { col, cells: span, text: text.to_string(), style });
        col += span;
    }
    runs
}

fn extend(runs: &mut Vec<Run>, next: Run) {
    match runs.last_mut() {
        Some(run) if run.style == next.style => {
            run.text.push_str(&next.text);
            run.cells += next.cells;
        }
        _ => runs.push(next),
    }
}

fn cursor_at(grid: &Grid, row: u16, cursor: bool) -> Option<usize> {
    grid.cursor.filter(|&(at, _)| cursor && at == usize::from(row)).map(|(_, col)| col)
}

fn style(glyph: &Glyph, palette: &Palette, cursor: bool) -> Style {
    let (fg, bg) = cell_colors(glyph, palette, cursor);
    Style { fg, bg, font: Font { bold: glyph.emphasis.bold, italic: glyph.emphasis.italic }, lines: glyph.decoration }
}

fn escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

pub(crate) fn svg(drawing: &Drawing<'_>, refs: &[Ref]) -> Result<Svg> {
    let Drawing { grid, look, .. } = *drawing;
    let Framed { palette, layout, shown, legend, size: (width, height) } = drawing.framed(refs)?;
    let placed =
        shown.iter().map(|(index, r)| Ok((*index, *r, Placed::new(&layout, r)?))).collect::<Result<Vec<_>>>()?;
    let text = fmt::from_fn(|f| {
        write_header(f, (width, height), &layout, palette)?;
        for (row, cells) in layout.rows_of(grid) {
            write_row(f, (cells, cursor_at(grid, row, look.cursor)), (&layout, palette), row)?;
        }
        for (index, r, placed) in &placed {
            write_ref(f, palette, *index, r, placed)?;
        }
        write_legend(f, &layout, palette, &legend)?;
        f.write_str("</svg>\n")
    })
    .to_string();
    Ok(Svg { text, width, height })
}

fn write_header(
    f: &mut fmt::Formatter<'_>,
    (width, height): (u32, u32),
    layout: &Layout,
    palette: &Palette,
) -> fmt::Result {
    write!(
        f,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" font-family=\"{FONT_FAMILY}\" font-size=\"{}\">\n<rect width=\"100%\" height=\"100%\" fill=\"{}\"/>\n",
        layout.cell.px,
        palette.background.css()
    )
}

fn write_row(
    f: &mut fmt::Formatter<'_>,
    (cells, cursor): (&[Glyph], Option<usize>),
    (layout, palette): (&Layout, &Palette),
    row: u16,
) -> fmt::Result {
    let cell = &layout.cell;
    let y = layout.y(row);
    for run in runs(cells, palette, cursor) {
        let x = layout.x(run.col);
        let run_width = layout.cells_wide(run.cells);
        if run.style.bg != palette.background {
            writeln!(
                f,
                "<rect x=\"{x}\" y=\"{y}\" width=\"{run_width}\" height=\"{}\" fill=\"{}\"/>",
                cell.height,
                run.style.bg.css()
            )?;
        }
        if run.text.trim().is_empty() && run.style.decorations().is_empty() {
            continue;
        }
        writeln!(
            f,
            "<text x=\"{x}\" y=\"{}\" textLength=\"{run_width}\" lengthAdjust=\"spacingAndGlyphs\" xml:space=\"preserve\" {}>{}</text>",
            y + u32::from(cell.baseline),
            attributes(&run.style),
            escaped(&run.text)
        )?;
    }
    Ok(())
}

fn attributes(style: &Style) -> String {
    let weight = if style.font.bold { " font-weight=\"bold\"" } else { "" };
    let slant = if style.font.italic { " font-style=\"italic\"" } else { "" };
    let decorations = style.decorations();
    let decoration =
        if decorations.is_empty() { String::new() } else { format!(" text-decoration=\"{}\"", decorations.join(" ")) };
    format!("fill=\"{}\"{weight}{slant}{decoration}", style.fg.css())
}

fn write_ref(f: &mut fmt::Formatter<'_>, palette: &Palette, index: usize, r: &Ref, placed: &Placed) -> fmt::Result {
    let color = palette.ref_color(index).css();
    let half = f64::from(placed.line) / 2.0;
    let (x, y) = (f64::from(placed.left) + half, f64::from(placed.top) + half);
    let (box_width, box_height) = (f64::from(placed.right) - half - x, f64::from(placed.bottom) - half - y);
    let line = placed.line;
    writeln!(
        f,
        "<rect x=\"{x}\" y=\"{y}\" width=\"{box_width}\" height=\"{box_height}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{line}\"/>"
    )?;
    let tag = &placed.tag;
    writeln!(
        f,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{color}\"/><text x=\"{}\" y=\"{}\" font-size=\"{}\" fill=\"{}\">{}</text>",
        tag.x,
        tag.y,
        tag.width,
        tag.cell.height,
        tag.x + u32::from(line),
        tag.y + u32::from(tag.cell.baseline),
        tag.cell.px,
        palette.background.css(),
        escaped(&r.id)
    )
}

fn write_legend(f: &mut fmt::Formatter<'_>, layout: &Layout, palette: &Palette, legend: &[Vec<Entry>]) -> fmt::Result {
    let cell = &layout.cell;
    for (line, entries) in (0..).zip(legend) {
        let y = layout.legend_top() + line * u32::from(cell.height);
        for entry in entries {
            let x = layout.x(entry.col);
            let tag_width = layout.cells_wide(entry.tag_cells());
            writeln!(
                f,
                "<rect x=\"{x}\" y=\"{y}\" width=\"{tag_width}\" height=\"{}\" fill=\"{}\"/><text x=\"{x}\" y=\"{}\" fill=\"{}\">{}</text><text x=\"{}\" y=\"{}\" fill=\"{}\" xml:space=\"preserve\">{}</text>",
                cell.height,
                palette.ref_color(entry.index).css(),
                y + u32::from(cell.baseline),
                palette.background.css(),
                escaped(&entry.tag),
                x + tag_width + u32::from(cell.width),
                y + u32::from(cell.baseline),
                palette.foreground.css(),
                escaped(&entry.rest)
            )?;
        }
    }
    Ok(())
}

pub(crate) fn html(drawing: &Drawing<'_>) -> Result<String> {
    let Drawing { grid, rows, look } = *drawing;
    let palette = theme::palette(look.theme);
    let layout = Layout::new(grid, rows, look.scale)?;
    let body = fmt::from_fn(|f| {
        layout
            .rows_of(grid)
            .try_for_each(|(row, cells)| write_html_row(f, (cells, cursor_at(grid, row, look.cursor)), palette))
    })
    .to_string();
    Ok(format!(
        "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>agent-terminal screenshot</title>\n<style>\nbody {{ margin: 0; background: {bg}; }}\npre {{ margin: 0; padding: {pad}px; color: {fg}; background: {bg}; font: {px}px/{line}px {FONT_FAMILY}; }}\n</style>\n</head>\n<body>\n<pre>{body}</pre>\n</body>\n</html>\n",
        bg = palette.background.css(),
        fg = palette.foreground.css(),
        pad = layout.pad,
        px = layout.cell.px,
        line = layout.cell.height,
    ))
}

fn write_html_row(
    f: &mut fmt::Formatter<'_>,
    (cells, cursor): (&[Glyph], Option<usize>),
    palette: &Palette,
) -> fmt::Result {
    let mut runs = runs(cells, palette, cursor);
    trim_blank_end(&mut runs, palette);
    for run in runs.iter().filter(|run| !run.text.is_empty()) {
        let style = html_style(&run.style, palette);
        if style.is_empty() {
            f.write_str(&escaped(&run.text))?;
        } else {
            write!(f, "<span style=\"{}\">{}</span>", style.join(";"), escaped(&run.text))?;
        }
    }
    f.write_char('\n')
}

fn trim_blank_end(runs: &mut [Run], palette: &Palette) {
    if let Some(last) = runs.last_mut()
        && last.style.bg == palette.background
        && !last.style.lines.underline
        && !last.style.lines.strikeout
    {
        last.text.truncate(last.text.trim_end().len());
    }
}

fn html_style(style: &Style, palette: &Palette) -> Vec<String> {
    let mut css = Vec::new();
    if style.fg != palette.foreground {
        css.push(format!("color:{}", style.fg.css()));
    }
    if style.bg != palette.background {
        css.push(format!("background:{}", style.bg.css()));
    }
    if style.font.bold {
        css.push("font-weight:bold".to_string());
    }
    if style.font.italic {
        css.push("font-style:italic".to_string());
    }
    let decorations = style.decorations();
    if !decorations.is_empty() {
        css.push(format!("text-decoration:{}", decorations.join(" ")));
    }
    css
}

#[cfg(test)]
mod tests;
