use super::theme::{self, Palette, Rgb};
use super::{Layout, Placed, cell_colors, legend_lines};
use crate::native::refs::{Glyph, Grid};
use crate::protocol::{Look, Ref, RowRange};

const FONT_FAMILY: &str = "'JetBrains Mono', 'JetBrainsMono Nerd Font', ui-monospace, Menlo, Consolas, monospace";

pub struct Svg {
    pub text: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, PartialEq)]
struct Style {
    fg: Rgb,
    bg: Rgb,
    bold: bool,
    italic: bool,
    underline: bool,
    strikeout: bool,
}

struct Run {
    col: usize,
    cells: usize,
    text: String,
    style: Style,
}

fn runs(grid: &Grid, row: usize, palette: &Palette, cursor: bool) -> Vec<Run> {
    let cells = &grid.rows[row];
    let mut runs: Vec<Run> = Vec::new();
    let mut col = 0;
    while col < cells.len() {
        let glyph: &Glyph = &cells[col];
        let span = if glyph.wide && col + 1 < cells.len() { 2 } else { 1 };
        let (fg, bg) = cell_colors(glyph, palette, cursor && grid.cursor == Some((row, col)));
        let style = Style {
            fg,
            bg,
            bold: glyph.bold,
            italic: glyph.italic,
            underline: glyph.underline,
            strikeout: glyph.strikeout,
        };
        let text = if glyph.text.is_empty() { " " } else { glyph.text.as_str() };
        match runs.last_mut() {
            Some(run) if run.style == style => {
                run.text.push_str(text);
                run.cells += span;
            }
            _ => runs.push(Run { col, cells: span, text: text.to_string(), style }),
        }
        col += span;
    }
    runs
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

pub fn svg(grid: &Grid, rows: RowRange, look: &Look, refs: &[Ref]) -> Svg {
    let palette = theme::palette(look.theme);
    let layout = Layout::new(grid, rows, look.scale);
    let cell = layout.cell;
    let shown: Vec<(usize, &Ref)> = refs.iter().enumerate().filter(|(_, r)| layout.shows(r)).collect();
    let legend = legend_lines(&shown, layout.cols.max(20));
    let legend_height = if legend.is_empty() { 0 } else { layout.pad + legend.len() as u32 * cell.height };
    let width = layout.grid_width().max(if legend.is_empty() { 0 } else { 20 * cell.width }) + 2 * layout.pad;
    let height = layout.grid_height() + 2 * layout.pad + legend_height;
    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" font-family=\"{FONT_FAMILY}\" font-size=\"{}\">\n<rect width=\"100%\" height=\"100%\" fill=\"{}\"/>\n",
        cell.px,
        palette.background.css()
    ));
    for row in layout.first..layout.last {
        let y = layout.y(row);
        for run in runs(grid, row, palette, look.cursor) {
            let x = layout.x(run.col);
            let run_width = run.cells as u32 * cell.width;
            if run.style.bg != palette.background {
                out.push_str(&format!(
                    "<rect x=\"{x}\" y=\"{y}\" width=\"{run_width}\" height=\"{}\" fill=\"{}\"/>\n",
                    cell.height,
                    run.style.bg.css()
                ));
            }
            if run.text.trim().is_empty() {
                continue;
            }
            let mut attributes = format!("fill=\"{}\"", run.style.fg.css());
            if run.style.bold {
                attributes.push_str(" font-weight=\"bold\"");
            }
            if run.style.italic {
                attributes.push_str(" font-style=\"italic\"");
            }
            let decorations: Vec<&str> = [(run.style.underline, "underline"), (run.style.strikeout, "line-through")]
                .into_iter()
                .filter_map(|(on, name)| on.then_some(name))
                .collect();
            if !decorations.is_empty() {
                attributes.push_str(&format!(" text-decoration=\"{}\"", decorations.join(" ")));
            }
            out.push_str(&format!(
                "<text x=\"{x}\" y=\"{}\" textLength=\"{run_width}\" lengthAdjust=\"spacingAndGlyphs\" xml:space=\"preserve\" {attributes}>{}</text>\n",
                y + cell.baseline,
                escaped(&run.text)
            ));
        }
    }
    for (index, r) in &shown {
        let color = palette.ref_color(*index).css();
        let placed = Placed::new(&layout, r);
        let half = placed.line as f32 / 2.0;
        let (x, y) = (placed.left as f32 + half, placed.top as f32 + half);
        let (box_width, box_height) = (placed.right as f32 - half - x, placed.bottom as f32 - half - y);
        let line = placed.line;
        out.push_str(&format!(
            "<rect x=\"{x}\" y=\"{y}\" width=\"{box_width}\" height=\"{box_height}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{line}\"/>\n"
        ));
        let tag = &placed.tag;
        out.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{color}\"/><text x=\"{}\" y=\"{}\" font-size=\"{}\" fill=\"{}\">{}</text>\n",
            tag.x,
            tag.y,
            tag.width,
            tag.cell.height,
            tag.x + line,
            tag.y + tag.cell.baseline,
            tag.cell.px,
            palette.background.css(),
            escaped(&r.id)
        ));
    }
    let legend_top = layout.pad * 2 + layout.grid_height();
    for (line, entries) in legend.iter().enumerate() {
        let y = legend_top + line as u32 * cell.height;
        for entry in entries {
            let x = layout.x(entry.col);
            let tag_width = entry.tag.chars().count() as u32 * cell.width;
            out.push_str(&format!(
                "<rect x=\"{x}\" y=\"{y}\" width=\"{tag_width}\" height=\"{}\" fill=\"{}\"/><text x=\"{x}\" y=\"{}\" fill=\"{}\">{}</text><text x=\"{}\" y=\"{}\" fill=\"{}\" xml:space=\"preserve\">{}</text>\n",
                cell.height,
                palette.ref_color(entry.index).css(),
                y + cell.baseline,
                palette.background.css(),
                escaped(&entry.tag),
                x + tag_width + cell.width,
                y + cell.baseline,
                palette.foreground.css(),
                escaped(&entry.rest)
            ));
        }
    }
    out.push_str("</svg>\n");
    Svg { text: out, width, height }
}

pub fn html(grid: &Grid, rows: RowRange, look: &Look) -> String {
    let palette = theme::palette(look.theme);
    let layout = Layout::new(grid, rows, look.scale);
    let cell = layout.cell;
    let mut body = String::new();
    for row in layout.first..layout.last {
        let mut runs = runs(grid, row, palette, look.cursor);
        if let Some(last) = runs.last_mut()
            && last.style.bg == palette.background
            && !last.style.underline
            && !last.style.strikeout
        {
            last.text.truncate(last.text.trim_end().len());
        }
        for run in runs.iter().filter(|run| !run.text.is_empty()) {
            let mut style = Vec::new();
            if run.style.fg != palette.foreground {
                style.push(format!("color:{}", run.style.fg.css()));
            }
            if run.style.bg != palette.background {
                style.push(format!("background:{}", run.style.bg.css()));
            }
            if run.style.bold {
                style.push("font-weight:bold".to_string());
            }
            if run.style.italic {
                style.push("font-style:italic".to_string());
            }
            let decorations: Vec<&str> = [(run.style.underline, "underline"), (run.style.strikeout, "line-through")]
                .into_iter()
                .filter_map(|(on, name)| on.then_some(name))
                .collect();
            if !decorations.is_empty() {
                style.push(format!("text-decoration:{}", decorations.join(" ")));
            }
            if style.is_empty() {
                body.push_str(&escaped(&run.text));
            } else {
                body.push_str(&format!("<span style=\"{}\">{}</span>", style.join(";"), escaped(&run.text)));
            }
        }
        body.push('\n');
    }
    format!(
        "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>agent-terminal screenshot</title>\n<style>\nbody {{ margin: 0; background: {bg}; }}\npre {{ margin: 0; padding: {pad}px; color: {fg}; background: {bg}; font: {px}px/{line}px {FONT_FAMILY}; }}\n</style>\n</head>\n<body>\n<pre>{body}</pre>\n</body>\n</html>\n",
        bg = palette.background.css(),
        fg = palette.foreground.css(),
        pad = layout.pad,
        px = cell.px,
        line = cell.height,
    )
}
