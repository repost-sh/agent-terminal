mod boxes;
mod diff;
mod fonts;
mod theme;
mod vector;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use crate::native::refs::{Glyph, Grid};
use crate::protocol::{Look, Ref, RowRange};
pub use diff::compare;
use fonts::{CellSize, Face, GLYPHS, Glyphs, Mask};
use theme::{Palette, Rgb};
pub use vector::{html, svg};

const PAD: u32 = 10;
const TAG_SCALE: f32 = 0.7;

pub struct Image {
    pixmap: Pixmap,
}

impl Image {
    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    pub fn png(&self) -> Result<Vec<u8>> {
        let rgb: Vec<u8> =
            self.pixmap.data().chunks_exact(4).flat_map(|pixel| [pixel[0], pixel[1], pixel[2]]).collect();
        let mut png = Vec::new();
        let mut encoder = png::Encoder::new(&mut png, self.width(), self.height());
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header().context("write the PNG header")?;
        writer.write_image_data(&rgb).context("encode the PNG")?;
        writer.finish().context("finish the PNG")?;
        Ok(png)
    }
}

pub fn check_fonts() -> Result<Vec<&'static str>, String> {
    let glyphs = GLYPHS.lock().unwrap();
    let samples = [
        (Face::Regular, 'A'),
        (Face::Bold, 'A'),
        (Face::Italic, 'A'),
        (Face::BoldItalic, 'A'),
        (Face::Symbols, '\u{e0b0}'),
        (Face::Emoji, '\u{1f600}'),
    ];
    let missing: Vec<&str> =
        samples.iter().filter(|(face, c)| !glyphs.covers(*face, *c)).map(|(face, _)| face.name()).collect();
    if missing.is_empty() {
        Ok(vec![Face::Regular.name(), Face::Symbols.name(), Face::Emoji.name()])
    } else {
        Err(missing.join(", "))
    }
}

struct Layout {
    cell: CellSize,
    pad: u32,
    cols: usize,
    first: usize,
    last: usize,
}

impl Layout {
    fn new(grid: &Grid, rows: RowRange, scale: u8) -> Self {
        let first = (rows.first as usize).saturating_sub(1).min(grid.rows.len());
        let last = (rows.last as usize).min(grid.rows.len()).max(first);
        Self {
            cell: CellSize::at(scale),
            pad: PAD * u32::from(scale.max(1)),
            cols: grid.rows.first().map_or(0, Vec::len),
            first,
            last,
        }
    }

    fn x(&self, col: usize) -> u32 {
        self.pad + col as u32 * self.cell.width
    }

    fn y(&self, row: usize) -> u32 {
        self.pad + (row - self.first) as u32 * self.cell.height
    }

    fn grid_width(&self) -> u32 {
        self.cols as u32 * self.cell.width
    }

    fn grid_height(&self) -> u32 {
        (self.last - self.first) as u32 * self.cell.height
    }

    fn shows(&self, r: &Ref) -> bool {
        r.rows[1] as usize > self.first && (r.rows[0] as usize) <= self.last
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cells {
    pub scale: u8,
    pub cols: u16,
    pub rows: RowRange,
}

pub fn cells(grid: &Grid, rows: RowRange, look: &Look) -> Cells {
    let layout = Layout::new(grid, rows, look.scale);
    Cells {
        scale: look.scale.max(1),
        cols: layout.cols as u16,
        rows: RowRange { first: layout.first as u16 + 1, last: layout.last as u16 },
    }
}

pub fn image(grid: &Grid, rows: RowRange, look: &Look, refs: &[Ref]) -> Image {
    let palette = theme::palette(look.theme);
    let layout = Layout::new(grid, rows, look.scale);
    let shown: Vec<(usize, &Ref)> = refs.iter().enumerate().filter(|(_, r)| layout.shows(r)).collect();
    let legend = legend_lines(&shown, layout.cols.max(20));
    let legend_height = if legend.is_empty() { 0 } else { layout.pad + legend.len() as u32 * layout.cell.height };
    let width = layout.grid_width().max(if legend.is_empty() { 0 } else { 20 * layout.cell.width }) + 2 * layout.pad;
    let height = layout.grid_height() + 2 * layout.pad + legend_height;
    let mut canvas = Canvas::new(width, height, palette.background);
    let mut glyphs = GLYPHS.lock().unwrap();
    for row in layout.first..layout.last {
        canvas.row(&mut glyphs, &layout, palette, grid, row, look.cursor);
    }
    for (index, r) in &shown {
        canvas.ref_box(&mut glyphs, &layout, palette, *index, r);
    }
    let legend_top = layout.pad * 2 + layout.grid_height();
    for (line, entries) in legend.iter().enumerate() {
        let y = legend_top + line as u32 * layout.cell.height;
        for entry in entries {
            let x = layout.x(entry.col);
            let color = palette.ref_color(entry.index);
            canvas.fill(x, y, entry.tag.chars().count() as u32 * layout.cell.width, layout.cell.height, color);
            canvas.text(&mut glyphs, x, y, &entry.tag, &layout.cell, palette.background);
            let after = x + (entry.tag.chars().count() as u32 + 1) * layout.cell.width;
            canvas.text(&mut glyphs, after, y, &entry.rest, &layout.cell, palette.foreground);
        }
    }
    Image { pixmap: canvas.pixmap }
}

struct Entry {
    index: usize,
    col: usize,
    tag: String,
    rest: String,
}

fn legend_lines(shown: &[(usize, &Ref)], cols: usize) -> Vec<Vec<Entry>> {
    let mut lines: Vec<Vec<Entry>> = Vec::new();
    let mut col = 0;
    for (index, r) in shown {
        let tag = r.id.clone();
        let rest = format!("{} \"{}\"", r.kind, r.label);
        let length = tag.chars().count() + 1 + rest.chars().count();
        if lines.is_empty() || (col > 0 && col + length > cols) {
            lines.push(Vec::new());
            col = 0;
        }
        lines.last_mut().expect("a legend line").push(Entry { index: *index, col, tag, rest });
        col += length + 3;
    }
    lines
}

fn cell_colors(glyph: &Glyph, palette: &Palette, cursor: bool) -> (Rgb, Rgb) {
    let mut fg = palette.color(&glyph.fg, palette.foreground);
    let mut bg = palette.color(&glyph.bg, palette.background);
    if glyph.inverse {
        std::mem::swap(&mut fg, &mut bg);
    }
    if glyph.dim {
        fg = fg.mix(bg, 0.4);
    }
    if cursor {
        std::mem::swap(&mut fg, &mut bg);
    }
    (fg, bg)
}

struct Canvas {
    pixmap: Pixmap,
}

impl Canvas {
    fn new(width: u32, height: u32, background: Rgb) -> Self {
        let mut pixmap = Pixmap::new(width.max(1), height.max(1)).expect("an image of at least one pixel");
        pixmap.fill(Color::from_rgba8(background.0, background.1, background.2, 255));
        Self { pixmap }
    }

    fn row(&mut self, glyphs: &mut Glyphs, layout: &Layout, palette: &Palette, grid: &Grid, row: usize, cursor: bool) {
        let cells = &grid.rows[row];
        let cell = &layout.cell;
        let y = layout.y(row);
        let mut col = 0;
        while col < cells.len() {
            let glyph = &cells[col];
            let span = if glyph.wide && col + 1 < cells.len() { 2 } else { 1 };
            let x = layout.x(col);
            let width = span * cell.width;
            let (fg, bg) = cell_colors(glyph, palette, cursor && grid.cursor == Some((row, col)));
            if bg != palette.background {
                self.fill(x, y, width, cell.height, bg);
            }
            if !glyph.text.is_empty() && glyph.text != " " {
                let face = Face::styled(glyph.bold, glyph.italic);
                match glyphs.mask(&glyph.text, face, cell, span) {
                    Some(mask) => self.blend(x, y, &mask, fg),
                    None => self.missing(x, y, width, cell, fg),
                }
            }
            if glyph.underline {
                self.fill(x, y + cell.underline, width, cell.stroke, fg);
            }
            if glyph.strikeout {
                self.fill(x, y + cell.strikeout, width, cell.stroke, fg);
            }
            col += span as usize;
        }
    }

    fn missing(&mut self, x: u32, y: u32, width: u32, cell: &CellSize, color: Rgb) {
        let line = cell.stroke;
        let (left, right) = (x + line, x + width - 2 * line);
        let (top, bottom) = (y + cell.height / 5, y + cell.baseline);
        self.fill(left, top, right - left, line, color);
        self.fill(left, bottom - line, right - left, line, color);
        self.fill(left, top, line, bottom - top, color);
        self.fill(right - line, top, line, bottom - top, color);
    }

    fn text(&mut self, glyphs: &mut Glyphs, x: u32, y: u32, text: &str, cell: &CellSize, color: Rgb) {
        for (i, c) in text.chars().enumerate() {
            if c == ' ' {
                continue;
            }
            if let Some(mask) = glyphs.mask(c.encode_utf8(&mut [0u8; 4]), Face::Regular, cell, 1) {
                self.blend(x + i as u32 * cell.width, y, &mask, color);
            }
        }
    }

    fn fill(&mut self, x: u32, y: u32, width: u32, height: u32, color: Rgb) {
        let stride = self.pixmap.width() as usize;
        let right = (x + width).min(self.pixmap.width()) as usize;
        let bottom = (y + height).min(self.pixmap.height()) as usize;
        let data = self.pixmap.data_mut();
        for row in y as usize..bottom {
            for col in x as usize..right {
                let at = (row * stride + col) * 4;
                data[at..at + 4].copy_from_slice(&[color.0, color.1, color.2, 255]);
            }
        }
    }

    fn blend(&mut self, x: u32, y: u32, mask: &Mask, color: Rgb) {
        let (width, height) = (self.pixmap.width() as i32, self.pixmap.height() as i32);
        let data = self.pixmap.data_mut();
        for my in 0..mask.height as i32 {
            let py = y as i32 + mask.top + my;
            if !(0..height).contains(&py) {
                continue;
            }
            for mx in 0..mask.width as i32 {
                let px = x as i32 + mask.left + mx;
                let alpha = u32::from(mask.alpha[(my * mask.width as i32 + mx) as usize]);
                if alpha == 0 || !(0..width).contains(&px) {
                    continue;
                }
                let at = ((py * width + px) * 4) as usize;
                let mix = |under: u8, over: u8| {
                    ((u32::from(over) * alpha + u32::from(under) * (255 - alpha) + 127) / 255) as u8
                };
                data[at] = mix(data[at], color.0);
                data[at + 1] = mix(data[at + 1], color.1);
                data[at + 2] = mix(data[at + 2], color.2);
            }
        }
    }

    fn ref_box(&mut self, glyphs: &mut Glyphs, layout: &Layout, palette: &Palette, index: usize, r: &Ref) {
        let color = palette.ref_color(index);
        let placed = Placed::new(layout, r);
        let half = placed.line as f32 / 2.0;
        let (left, top) = (placed.left as f32 + half, placed.top as f32 + half);
        if let Some(rect) = Rect::from_ltrb(left, top, placed.right as f32 - half, placed.bottom as f32 - half) {
            let mut paint = Paint::default();
            paint.set_color_rgba8(color.0, color.1, color.2, 255);
            paint.anti_alias = false;
            let stroke = Stroke { width: placed.line as f32, ..Stroke::default() };
            self.pixmap.stroke_path(&PathBuilder::from_rect(rect), &paint, &stroke, Transform::identity(), None);
        }
        let tag = &placed.tag;
        self.fill(tag.x, tag.y, tag.width, tag.cell.height, color);
        self.text(glyphs, tag.x + placed.line, tag.y, &r.id, &tag.cell, palette.background);
    }
}

struct Placed {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    line: u32,
    tag: Tag,
}

struct Tag {
    x: u32,
    y: u32,
    width: u32,
    cell: CellSize,
}

impl Placed {
    fn new(layout: &Layout, r: &Ref) -> Self {
        let first_row = (r.rows[0] as usize - 1).max(layout.first);
        let last_row = (r.rows[1] as usize - 1).min(layout.last - 1);
        let (left, right) = (layout.x(r.cols[0] as usize - 1), layout.x(r.cols[1] as usize));
        let (top, bottom) = (layout.y(first_row), layout.y(last_row) + layout.cell.height);
        let line = (layout.cell.px / 7.5).round().max(1.0) as u32;
        let cell = CellSize::for_px(layout.cell.px * TAG_SCALE);
        let width = r.id.chars().count() as u32 * cell.width + 2 * line;
        let y = if top >= cell.height + layout.pad / 2 { top - cell.height } else { top };
        let tag = Tag { x: right.saturating_sub(width), y, width, cell };
        Self { left, top, right, bottom, line, tag }
    }
}
