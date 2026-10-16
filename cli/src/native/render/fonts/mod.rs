use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use anyhow::{Context, Result};
use num_traits::ToPrimitive;
use swash::FontRef;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Format, Vector};

use super::boxes::{self, BoxCell};

const FONT_PX: f32 = 15.0;

macro_rules! font_file {
    ($name:literal) => {
        ($name, include_bytes!(concat!("../../../../assets/fonts/", $name)).as_slice())
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Face {
    Regular,
    Bold,
    Italic,
    BoldItalic,
    Symbols,
    Emoji,
}

impl Face {
    pub(crate) const ALL: [Face; 6] =
        [Face::Regular, Face::Bold, Face::Italic, Face::BoldItalic, Face::Symbols, Face::Emoji];

    pub(crate) fn styled(bold: bool, italic: bool) -> Self {
        match (bold, italic) {
            (false, false) => Face::Regular,
            (true, false) => Face::Bold,
            (false, true) => Face::Italic,
            (true, true) => Face::BoldItalic,
        }
    }

    fn file(self) -> (&'static str, &'static [u8]) {
        match self {
            Face::Regular => font_file!("JetBrainsMonoNL-Regular.ttf"),
            Face::Bold => font_file!("JetBrainsMonoNL-Bold.ttf"),
            Face::Italic => font_file!("JetBrainsMonoNL-Italic.ttf"),
            Face::BoldItalic => font_file!("JetBrainsMonoNL-BoldItalic.ttf"),
            Face::Symbols => font_file!("SymbolsNerdFontMono-Regular.ttf"),
            Face::Emoji => font_file!("NotoEmoji-VariableFont_wght.ttf"),
        }
    }

    pub(crate) fn bytes(self) -> &'static [u8] {
        self.file().1
    }

    pub(crate) fn file_name(self) -> &'static str {
        self.file().0
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Face::Regular => "JetBrains Mono",
            Face::Bold => "JetBrains Mono Bold",
            Face::Italic => "JetBrains Mono Italic",
            Face::BoldItalic => "JetBrains Mono Bold Italic",
            Face::Symbols => "Symbols Nerd Font Mono",
            Face::Emoji => "Noto Emoji",
        }
    }

    pub(crate) fn font(self) -> Option<FontRef<'static>> {
        FontRef::from_index(self.bytes(), 0)
    }

    fn is_text(self) -> bool {
        matches!(self, Face::Regular | Face::Bold | Face::Italic | Face::BoldItalic)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Letters<'a> {
    pub text: &'a str,
    pub face: Face,
    pub span: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CellSize {
    pub width: u16,
    pub height: u16,
    pub baseline: u16,
    pub px: f32,
    pub underline: u16,
    pub strikeout: u16,
    pub stroke: u16,
}

impl CellSize {
    pub(crate) fn at(scale: u8) -> Result<Self> {
        Self::for_px(FONT_PX * f32::from(scale.max(1)))
    }

    pub(crate) fn for_px(px: f32) -> Result<Self> {
        let font = regular()?;
        let metrics = font.metrics(&[]).scale(px);
        let advance = font.glyph_metrics(&[]).scale(px).advance_width(font.charmap().map('M'));
        let natural = metrics.ascent + metrics.descent + metrics.leading;
        let height = natural.round().max(1.0);
        let baseline = (metrics.ascent + (height - natural) / 2.0).round();
        let stroke = metrics.stroke_size.round().max(1.0);
        let whole =
            |value: f32| value.to_u16().with_context(|| format!("the bundled font measures {value} pixels at {px}px"));
        Ok(Self {
            width: whole(advance.round().max(1.0))?,
            height: whole(height)?,
            baseline: whole(baseline)?,
            px,
            underline: whole((baseline - metrics.underline_offset).round().clamp(0.0, height - stroke))?,
            strikeout: whole((baseline - metrics.strikeout_offset).round().clamp(0.0, height - stroke))?,
            stroke: whole(stroke)?,
        })
    }
}

fn regular() -> Result<FontRef<'static>> {
    Face::Regular.font().context("the bundled JetBrains Mono does not parse")
}

pub(crate) struct Mask {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub alpha: Vec<u8>,
}

impl Mask {
    fn right(&self) -> i64 {
        i64::from(self.left) + i64::from(self.width)
    }

    fn bottom(&self) -> i64 {
        i64::from(self.top) + i64::from(self.height)
    }

    fn covering(base: &Mask, mark: &Mask) -> Option<Mask> {
        let (left, top) = (base.left.min(mark.left), base.top.min(mark.top));
        let width = u32::try_from(base.right().max(mark.right()) - i64::from(left)).ok()?;
        let height = u32::try_from(base.bottom().max(mark.bottom()) - i64::from(top)).ok()?;
        let alpha = vec![0u8; usize::try_from(u64::from(width) * u64::from(height)).ok()?];
        Some(Mask { left, top, width, height, alpha })
    }

    pub(crate) fn rows(&self) -> impl Iterator<Item = &[u8]> {
        let width = usize::try_from(self.width).unwrap_or(usize::MAX).max(1);
        self.alpha.chunks(width)
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    text: String,
    face: Face,
    px: u32,
    span: u16,
}

pub(crate) struct Glyphs {
    context: ScaleContext,
    fonts: Vec<(Face, FontRef<'static>)>,
    masks: HashMap<Key, Option<Arc<Mask>>>,
}

pub(crate) static GLYPHS: LazyLock<Mutex<Glyphs>> = LazyLock::new(|| Mutex::new(Glyphs::new()));

const VARIATION_SELECTORS: std::ops::RangeInclusive<char> = '\u{fe00}'..='\u{fe0f}';
const ZERO_WIDTH_JOINER: char = '\u{200d}';

fn is_combining(c: char) -> bool {
    matches!(c, '\u{0300}'..='\u{036f}' | '\u{1ab0}'..='\u{1aff}' | '\u{1dc0}'..='\u{1dff}' | '\u{20d0}'..='\u{20ff}' | '\u{fe20}'..='\u{fe2f}')
}

impl Glyphs {
    fn new() -> Self {
        let fonts = Face::ALL.iter().filter_map(|face| Some((*face, face.font()?))).collect();
        Self { context: ScaleContext::new(), fonts, masks: HashMap::new() }
    }

    fn font(&self, face: Face) -> Option<FontRef<'static>> {
        self.fonts.iter().find(|(f, _)| *f == face).map(|(_, font)| *font)
    }

    pub(crate) fn covers(&self, face: Face, c: char) -> bool {
        self.font(face).is_some_and(|font| font.charmap().map(c) != 0)
    }

    pub(crate) fn mask(&mut self, letters: &Letters<'_>, cell: &CellSize) -> Option<Arc<Mask>> {
        let Letters { text, face, span } = *letters;
        let key = Key { text: text.to_string(), face, px: cell.px.to_bits(), span };
        if let Some(found) = self.masks.get(&key) {
            return found.clone();
        }
        let drawn = self.draw(text, face, cell, span).map(Arc::new);
        self.masks.insert(key, drawn.clone());
        drawn
    }

    fn draw(&mut self, text: &str, style: Face, cell: &CellSize, span: u16) -> Option<Mask> {
        let mut chars = text.chars().filter(|c| !VARIATION_SELECTORS.contains(c));
        let first = chars.next()?;
        let scale = (cell.px / FONT_PX).round().clamp(0.0, 255.0).to_u8().unwrap_or_default();
        if text.chars().count() == 1
            && let Some(mask) = boxes::mask(first, &BoxCell { width: cell.width * span, height: cell.height, scale })
        {
            return Some(mask);
        }
        let emoji_first = text.contains('\u{fe0f}');
        let face = self.face_for(first, style, emoji_first)?;
        let (mut drawn, pen) = self.glyph(face, first, cell, span)?;
        for mark in chars.take_while(|c| *c != ZERO_WIDTH_JOINER).filter(|c| is_combining(*c)) {
            if let Some(merged) = self
                .face_for(mark, style, false)
                .and_then(|face| self.font(face))
                .and_then(|font| {
                    self.rendered(&Outline { font, id: font.charmap().map(mark), px: cell.px, at: pen }, cell)
                })
                .and_then(|mark| merged(&drawn, &mark))
            {
                drawn = merged;
            }
        }
        Some(drawn)
    }

    fn face_for(&self, c: char, style: Face, emoji_first: bool) -> Option<Face> {
        let order: [Face; 4] = if emoji_first {
            [Face::Emoji, style, Face::Regular, Face::Symbols]
        } else {
            [style, Face::Regular, Face::Symbols, Face::Emoji]
        };
        order.into_iter().find(|face| self.covers(*face, c))
    }

    fn glyph(&mut self, face: Face, c: char, cell: &CellSize, span: u16) -> Option<(Mask, f32)> {
        let font = self.font(face)?;
        let id = font.charmap().map(c);
        let room = f32::from(cell.width * span);
        let natural = font.glyph_metrics(&[]).scale(cell.px).advance_width(id);
        let px = fitted_px(face, natural, room, cell.px);
        let advance = if face.is_text() && span == 1 { room } else { natural * px / cell.px };
        let shift = ((room - advance) / 2.0).max(0.0);
        Some((self.rendered(&Outline { font, id, px, at: shift }, cell)?, shift + advance))
    }

    fn rendered(&mut self, outline: &Outline, cell: &CellSize) -> Option<Mask> {
        let Outline { font, id, px, at } = *outline;
        let mut scaler = self.context.builder(font).size(px).hint(false).build();
        let image = Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .offset(Vector::new(at.fract(), 0.0))
            .render(&mut scaler, id)?;
        Some(Mask {
            left: at.trunc().to_i32()? + image.placement.left,
            top: i32::from(cell.baseline) - image.placement.top,
            width: image.placement.width,
            height: image.placement.height,
            alpha: image.data,
        })
    }
}

#[derive(Clone, Copy)]
struct Outline {
    font: FontRef<'static>,
    id: swash::GlyphId,
    px: f32,
    at: f32,
}

fn merged(base: &Mask, mark: &Mask) -> Option<Mask> {
    let mut merged = Mask::covering(base, mark)?;
    merged.overlay(base)?;
    merged.overlay(mark)?;
    Some(merged)
}

impl Mask {
    fn stride(&self) -> Option<usize> {
        usize::try_from(self.width).ok()
    }

    fn overlay(&mut self, part: &Mask) -> Option<()> {
        let stride = self.stride()?;
        let corner = (usize::try_from(part.left - self.left).ok()?, usize::try_from(part.top - self.top).ok()?);
        paint(&mut self.alpha, stride, corner, part)
    }
}

fn paint(alpha: &mut [u8], stride: usize, (across, down): (usize, usize), part: &Mask) -> Option<()> {
    for (y, row) in part.rows().enumerate() {
        let start = (down + y) * stride + across;
        let target = alpha.get_mut(start..start + row.len())?;
        target.iter_mut().zip(row).for_each(|(slot, value)| *slot = (*slot).max(*value));
    }
    Some(())
}

fn fitted_px(face: Face, natural: f32, room: f32, px: f32) -> f32 {
    if !face.is_text() && natural > room && natural > 0.0 { px * room / natural } else { px }
}

#[cfg(test)]
mod tests;
