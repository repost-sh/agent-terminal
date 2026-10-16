mod boxes;
mod diff;
pub(crate) mod fonts;
mod theme;
mod vector;

use std::sync::PoisonError;

use anyhow::{Context, Result};
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use crate::native::refs::{Decoration, Glyph, Grid};
use crate::protocol::{Look, Ref, RowRange};
pub(crate) use diff::{CellBox, compare};
use fonts::{CellSize, Face, GLYPHS, Glyphs, Letters, Mask};
use theme::{Palette, Rgb};
pub(crate) use vector::{html, svg};

const PAD: u32 = 10;
const TAG_SCALE: f32 = 0.7;

pub(crate) struct Image {
    pixmap: Pixmap,
}

impl Image {
    pub(crate) fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub(crate) fn height(&self) -> u32 {
        self.pixmap.height()
    }

    pub(crate) fn png(&self) -> Result<Vec<u8>> {
        let rgb: Vec<u8> = self.pixmap.data().as_chunks::<4>().0.iter().flat_map(|&[r, g, b, _]| [r, g, b]).collect();
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

pub(crate) fn check_fonts() -> Result<Vec<&'static str>, String> {
    let glyphs = GLYPHS.lock().unwrap_or_else(PoisonError::into_inner);
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

#[derive(Clone, Copy)]
struct Span {
    cols: u16,
    first: u16,
    last: u16,
}

impl Span {
    fn of(grid: &Grid, rows: RowRange) -> Result<Self> {
        let height = u16::try_from(grid.rows.len()).context("the screen has more rows than a picture can show")?;
        let first = rows.first.saturating_sub(1).min(height);
        let last = rows.last.min(height).max(first);
        let cols = grid.rows.first().map_or(Ok(0), |row| u16::try_from(row.len()));
        Ok(Self { cols: cols.context("the screen has more columns than a picture can show")?, first, last })
    }
}

struct Layout {
    cell: CellSize,
    pad: u32,
    cols: u16,
    first: u16,
    last: u16,
}

impl Layout {
    fn new(grid: &Grid, rows: RowRange, scale: u8) -> Result<Self> {
        let Span { cols, first, last } = Span::of(grid, rows)?;
        Ok(Self { cell: CellSize::at(scale)?, pad: PAD * u32::from(scale.max(1)), cols, first, last })
    }

    fn rows_of<'g>(&self, grid: &'g Grid) -> impl Iterator<Item = (u16, &'g [Glyph])> {
        (0u16..)
            .zip(&grid.rows)
            .skip(usize::from(self.first))
            .take(usize::from(self.last.saturating_sub(self.first)))
            .map(|(row, cells)| (row, cells.as_slice()))
    }

    fn x(&self, col: u16) -> u32 {
        self.pad + u32::from(col) * u32::from(self.cell.width)
    }

    fn y(&self, row: u16) -> u32 {
        self.pad + u32::from(row - self.first) * u32::from(self.cell.height)
    }

    fn cells_wide(&self, count: u16) -> u32 {
        u32::from(count) * u32::from(self.cell.width)
    }

    fn grid_width(&self) -> u32 {
        self.cells_wide(self.cols)
    }

    fn grid_height(&self) -> u32 {
        u32::from(self.last - self.first) * u32::from(self.cell.height)
    }

    fn shows(&self, r: &Ref) -> bool {
        let [top, bottom] = r.rows;
        bottom > self.first && top <= self.last
    }

    fn picture_size(&self, legend: &[Vec<Entry>]) -> (u32, u32) {
        let lines = u32::try_from(legend.len()).unwrap_or(u32::MAX);
        let legend_height = if legend.is_empty() { 0 } else { self.pad + lines * u32::from(self.cell.height) };
        let legend_width = if legend.is_empty() { 0 } else { self.cells_wide(20) };
        let width = self.grid_width().max(legend_width) + 2 * self.pad;
        (width, self.grid_height() + 2 * self.pad + legend_height)
    }

    fn legend_top(&self) -> u32 {
        self.pad * 2 + self.grid_height()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Cells {
    pub scale: u8,
    pub cols: u16,
    pub rows: RowRange,
}

#[derive(Clone, Copy)]
pub(crate) struct Drawing<'a> {
    pub grid: &'a Grid,
    pub rows: RowRange,
    pub look: Look,
}

pub(crate) fn cells(drawing: &Drawing<'_>) -> Result<Cells> {
    let span = Span::of(drawing.grid, drawing.rows)?;
    Ok(Cells {
        scale: drawing.look.scale.max(1),
        cols: span.cols,
        rows: RowRange { first: span.first + 1, last: span.last },
    })
}

struct Framed<'r> {
    palette: &'static Palette,
    layout: Layout,
    shown: Vec<(usize, &'r Ref)>,
    legend: Vec<Vec<Entry>>,
    size: (u32, u32),
}

impl Drawing<'_> {
    fn framed<'r>(&self, refs: &'r [Ref]) -> Result<Framed<'r>> {
        let layout = Layout::new(self.grid, self.rows, self.look.scale)?;
        let shown: Vec<(usize, &Ref)> = refs.iter().enumerate().filter(|(_, r)| layout.shows(r)).collect();
        let legend = legend_lines(&shown, layout.cols.max(20));
        let size = layout.picture_size(&legend);
        Ok(Framed { palette: theme::palette(self.look.theme), layout, shown, legend, size })
    }
}

pub(crate) fn image(drawing: &Drawing<'_>, refs: &[Ref]) -> Result<Image> {
    let Drawing { grid, look, .. } = *drawing;
    let Framed { palette, layout, shown, legend, size: (width, height) } = drawing.framed(refs)?;
    let mut canvas = Canvas::new(width, height, palette.background)?;
    let mut glyphs = GLYPHS.lock().unwrap_or_else(PoisonError::into_inner);
    let painting = Painting { layout: &layout, palette, grid, cursor: look.cursor };
    for (row, cells) in layout.rows_of(grid) {
        canvas.row(&mut glyphs, &painting, row, cells);
    }
    for (index, r) in &shown {
        canvas.ref_box(&mut glyphs, &painting, *index, r)?;
    }
    for (line, entries) in (0..).zip(&legend) {
        let y = layout.legend_top() + line * u32::from(layout.cell.height);
        for entry in entries {
            canvas.legend_entry(&mut glyphs, &painting, y, entry);
        }
    }
    Ok(Image { pixmap: canvas.pixmap })
}

struct Entry {
    index: usize,
    col: u16,
    tag: String,
    rest: String,
}

impl Entry {
    fn tag_cells(&self) -> u16 {
        u16::try_from(self.tag.chars().count()).unwrap_or(u16::MAX)
    }
}

fn legend_lines(shown: &[(usize, &Ref)], cols: u16) -> Vec<Vec<Entry>> {
    let mut lines: Vec<Vec<Entry>> = Vec::new();
    let mut line: Vec<Entry> = Vec::new();
    let mut col: u16 = 0;
    for (index, r) in shown {
        let tag = r.id.clone();
        let rest = format!("{} \"{}\"", r.kind, r.label);
        let length = u16::try_from(tag.chars().count() + 1 + rest.chars().count()).unwrap_or(u16::MAX);
        if col > 0 && col.saturating_add(length) > cols {
            lines.push(std::mem::take(&mut line));
            col = 0;
        }
        line.push(Entry { index: *index, col, tag, rest });
        col = col.saturating_add(length).saturating_add(3);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn cell_colors(glyph: &Glyph, palette: &Palette, cursor: bool) -> (Rgb, Rgb) {
    let mut fg = palette.color(glyph.fg, palette.foreground);
    let mut bg = palette.color(glyph.bg, palette.background);
    if glyph.inverse {
        std::mem::swap(&mut fg, &mut bg);
    }
    if glyph.emphasis.dim {
        fg = fg.mix(bg, 0.4);
    }
    if cursor {
        std::mem::swap(&mut fg, &mut bg);
    }
    (fg, bg)
}

fn span_of(glyph: &Glyph, col: u16, cells: usize) -> u16 {
    if glyph.wide && usize::from(col) + 1 < cells { 2 } else { 1 }
}

struct Painting<'a> {
    layout: &'a Layout,
    palette: &'a Palette,
    grid: &'a Grid,
    cursor: bool,
}

struct Canvas {
    pixmap: Pixmap,
}

impl Canvas {
    fn new(width: u32, height: u32, background: Rgb) -> Result<Self> {
        let mut pixmap = Pixmap::new(width.max(1), height.max(1)).context("a picture of at least one pixel")?;
        pixmap.fill(Color::from_rgba8(background.0, background.1, background.2, 255));
        Ok(Self { pixmap })
    }

    fn row(&mut self, glyphs: &mut Glyphs, painting: &Painting<'_>, row: u16, cells: &[Glyph]) {
        let mut col = 0;
        while let Some(glyph) = cells.get(usize::from(col)) {
            let span = span_of(glyph, col, cells.len());
            let cursor = painting.cursor && painting.grid.cursor == Some((usize::from(row), usize::from(col)));
            let place = Place { x: painting.layout.x(col), y: painting.layout.y(row), span };
            self.cell(glyphs, painting, glyph, &place, cursor);
            col += span;
        }
    }

    fn cell(&mut self, glyphs: &mut Glyphs, painting: &Painting<'_>, glyph: &Glyph, place: &Place, cursor: bool) {
        let cell = &painting.layout.cell;
        let (x, y) = (place.x, place.y);
        let width = u32::from(place.span) * u32::from(cell.width);
        let (fg, bg) = cell_colors(glyph, painting.palette, cursor);
        if bg != painting.palette.background {
            self.fill(x, y, width, u32::from(cell.height), bg);
        }
        if !glyph.text.is_empty() && glyph.text != " " {
            let face = Face::styled(glyph.emphasis.bold, glyph.emphasis.italic);
            match glyphs.mask(&Letters { text: &glyph.text, face, span: place.span }, cell) {
                Some(mask) => self.blend(x, y, &mask, fg),
                None => self.missing(x, y, width, cell, fg),
            }
        }
        self.decorate(glyph.decoration, &Place { x, y, span: place.span }, cell, fg);
    }

    fn decorate(&mut self, decoration: Decoration, place: &Place, cell: &CellSize, color: Rgb) {
        let width = u32::from(place.span) * u32::from(cell.width);
        if decoration.underline {
            self.fill(place.x, place.y + u32::from(cell.underline), width, u32::from(cell.stroke), color);
        }
        if decoration.strikeout {
            self.fill(place.x, place.y + u32::from(cell.strikeout), width, u32::from(cell.stroke), color);
        }
    }

    fn missing(&mut self, x: u32, y: u32, width: u32, cell: &CellSize, color: Rgb) {
        let line = u32::from(cell.stroke);
        let (left, right) = (x + line, x + width - 2 * line);
        let (top, bottom) = (y + u32::from(cell.height) / 5, y + u32::from(cell.baseline));
        self.fill(left, top, right - left, line, color);
        self.fill(left, bottom - line, right - left, line, color);
        self.fill(left, top, line, bottom - top, color);
        self.fill(right - line, top, line, bottom - top, color);
    }

    fn text(&mut self, glyphs: &mut Glyphs, place: &Place, text: &str, cell: &CellSize, color: Rgb) {
        for (step, c) in (0..).zip(text.chars()) {
            if c == ' ' {
                continue;
            }
            if let Some(mask) =
                glyphs.mask(&Letters { text: c.encode_utf8(&mut [0u8; 4]), face: Face::Regular, span: 1 }, cell)
            {
                self.blend(place.x + step * u32::from(cell.width), place.y, &mask, color);
            }
        }
    }

    fn legend_entry(&mut self, glyphs: &mut Glyphs, painting: &Painting<'_>, y: u32, entry: &Entry) {
        let layout = painting.layout;
        let x = layout.x(entry.col);
        let color = painting.palette.ref_color(entry.index);
        self.fill(x, y, layout.cells_wide(entry.tag_cells()), u32::from(layout.cell.height), color);
        self.text(glyphs, &Place { x, y, span: 1 }, &entry.tag, &layout.cell, painting.palette.background);
        let after = x + layout.cells_wide(entry.tag_cells().saturating_add(1));
        self.text(glyphs, &Place { x: after, y, span: 1 }, &entry.rest, &layout.cell, painting.palette.foreground);
    }

    fn fill(&mut self, x: u32, y: u32, width: u32, height: u32, color: Rgb) {
        let stride = self.pixmap.width();
        let right = x.saturating_add(width).min(stride);
        let bottom = y.saturating_add(height).min(self.pixmap.height());
        let data = self.pixmap.data_mut();
        let rgba = [color.0, color.1, color.2, 255];
        for row in y..bottom {
            let span = offset(stride, x, row).zip(offset(stride, right, row));
            if let Some(pixels) = span.and_then(|(start, end)| data.get_mut(start..end)) {
                pixels.as_chunks_mut::<4>().0.iter_mut().for_each(|pixel| *pixel = rgba);
            }
        }
    }

    fn blend(&mut self, x: u32, y: u32, mask: &Mask, color: Rgb) {
        let (stride, height) = (self.pixmap.width(), self.pixmap.height());
        let data = self.pixmap.data_mut();
        let covered = (0_i64..)
            .zip(mask.rows())
            .flat_map(|(down, row)| (0_i64..).zip(row).map(move |(across, alpha)| (across, down, *alpha)));
        for (across, down, alpha) in covered.filter(|(_, _, alpha)| *alpha != 0) {
            let px = shifted(x, mask.left, across).filter(|&px| px < stride);
            let py = shifted(y, mask.top, down).filter(|&py| py < height);
            if let Some(pixel) = px.zip(py).and_then(|(px, py)| pixel(data, stride, px, py)) {
                blended(pixel, color, alpha);
            }
        }
    }

    fn ref_box(&mut self, glyphs: &mut Glyphs, painting: &Painting<'_>, index: usize, r: &Ref) -> Result<()> {
        let color = painting.palette.ref_color(index);
        let placed = Placed::new(painting.layout, r)?;
        if let Some(rect) = placed.outline() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(color.0, color.1, color.2, 255);
            paint.anti_alias = false;
            let stroke = Stroke { width: f32::from(placed.line), ..Stroke::default() };
            self.pixmap.stroke_path(&PathBuilder::from_rect(rect), &paint, &stroke, Transform::identity(), None);
        }
        let tag = &placed.tag;
        self.fill(tag.x, tag.y, tag.width, u32::from(tag.cell.height), color);
        let at = Place { x: tag.x + u32::from(placed.line), y: tag.y, span: 1 };
        self.text(glyphs, &at, &r.id, &tag.cell, painting.palette.background);
        Ok(())
    }
}

struct Place {
    x: u32,
    y: u32,
    span: u16,
}

fn shifted(base: u32, offset: i32, step: i64) -> Option<u32> {
    u32::try_from(i64::from(base) + i64::from(offset) + step).ok()
}

fn offset(stride: u32, x: u32, y: u32) -> Option<usize> {
    let at =
        usize::try_from(y).ok()?.checked_mul(usize::try_from(stride).ok()?)?.checked_add(usize::try_from(x).ok()?)?;
    at.checked_mul(4)
}

fn pixel(data: &mut [u8], stride: u32, x: u32, y: u32) -> Option<&mut [u8]> {
    let start = offset(stride, x, y)?;
    data.get_mut(start..start + 4)
}

fn blended(pixel: &mut [u8], color: Rgb, alpha: u8) {
    let alpha = u32::from(alpha);
    let mix = |under: u8, over: u8| {
        u8::try_from((u32::from(over) * alpha + u32::from(under) * (255 - alpha) + 127) / 255).unwrap_or(u8::MAX)
    };
    for (channel, over) in pixel.iter_mut().zip([color.0, color.1, color.2]) {
        *channel = mix(*channel, over);
    }
}

struct Placed {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    line: u16,
    tag: Tag,
}

struct Tag {
    x: u32,
    y: u32,
    width: u32,
    cell: CellSize,
}

impl Placed {
    fn new(layout: &Layout, r: &Ref) -> Result<Self> {
        let ([top_row, bottom_row], [first_col, last_col]) = (r.rows, r.cols);
        let first_row = top_row.saturating_sub(1).max(layout.first);
        let last_row = bottom_row.saturating_sub(1).min(layout.last.saturating_sub(1));
        let (left, right) = (layout.x(first_col.saturating_sub(1)), layout.x(last_col));
        let (top, bottom) = (layout.y(first_row), layout.y(last_row) + u32::from(layout.cell.height));
        let line = (layout.cell.px / 7.5).round().max(1.0).to_u16().context("the outline of a ref")?;
        let cell = CellSize::for_px(layout.cell.px * TAG_SCALE)?;
        let tag_cells = u32::try_from(r.id.chars().count()).unwrap_or(u32::MAX);
        let width = tag_cells * u32::from(cell.width) + 2 * u32::from(line);
        let tag_height = u32::from(cell.height);
        let y = if top >= tag_height + layout.pad / 2 { top - tag_height } else { top };
        let tag = Tag { x: right.saturating_sub(width), y, width, cell };
        Ok(Self { left, top, right, bottom, line, tag })
    }

    fn corners(&self) -> Option<[f32; 4]> {
        let half = f32::from(self.line) / 2.0;
        let exact = |value: u32| u16::try_from(value).ok().map(f32::from);
        Some([exact(self.left)? + half, exact(self.top)? + half, exact(self.right)? - half, exact(self.bottom)? - half])
    }

    fn outline(&self) -> Option<Rect> {
        let [left, top, right, bottom] = self.corners()?;
        Rect::from_ltrb(left, top, right, bottom)
    }
}
