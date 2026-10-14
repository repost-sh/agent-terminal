use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use swash::FontRef;
use swash::scale::image::Content;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Format, Vector};

use super::boxes;

const FONT_PX: f32 = 15.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    Regular,
    Bold,
    Italic,
    BoldItalic,
    Symbols,
    Emoji,
}

impl Face {
    pub fn styled(bold: bool, italic: bool) -> Self {
        match (bold, italic) {
            (false, false) => Face::Regular,
            (true, false) => Face::Bold,
            (false, true) => Face::Italic,
            (true, true) => Face::BoldItalic,
        }
    }

    fn data(self) -> &'static [u8] {
        match self {
            Face::Regular => include_bytes!("../../../assets/fonts/JetBrainsMonoNL-Regular.ttf"),
            Face::Bold => include_bytes!("../../../assets/fonts/JetBrainsMonoNL-Bold.ttf"),
            Face::Italic => include_bytes!("../../../assets/fonts/JetBrainsMonoNL-Italic.ttf"),
            Face::BoldItalic => include_bytes!("../../../assets/fonts/JetBrainsMonoNL-BoldItalic.ttf"),
            Face::Symbols => include_bytes!("../../../assets/fonts/SymbolsNerdFontMono-Regular.ttf"),
            Face::Emoji => include_bytes!("../../../assets/fonts/NotoEmoji-VariableFont_wght.ttf"),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Face::Regular => "JetBrains Mono",
            Face::Bold => "JetBrains Mono Bold",
            Face::Italic => "JetBrains Mono Italic",
            Face::BoldItalic => "JetBrains Mono Bold Italic",
            Face::Symbols => "Symbols Nerd Font Mono",
            Face::Emoji => "Noto Emoji",
        }
    }

    pub fn font(self) -> Option<FontRef<'static>> {
        FontRef::from_index(self.data(), 0)
    }

    fn is_text(self) -> bool {
        matches!(self, Face::Regular | Face::Bold | Face::Italic | Face::BoldItalic)
    }
}

pub const ALL: [Face; 6] = [Face::Regular, Face::Bold, Face::Italic, Face::BoldItalic, Face::Symbols, Face::Emoji];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellSize {
    pub width: u32,
    pub height: u32,
    pub baseline: u32,
    pub px: f32,
    pub underline: u32,
    pub strikeout: u32,
    pub stroke: u32,
}

impl CellSize {
    pub fn at(scale: u8) -> Self {
        Self::for_px(FONT_PX * f32::from(scale.max(1)))
    }

    pub fn for_px(px: f32) -> Self {
        let font = regular();
        let metrics = font.metrics(&[]).scale(px);
        let advance = font.glyph_metrics(&[]).scale(px).advance_width(font.charmap().map('M'));
        let natural = metrics.ascent + metrics.descent + metrics.leading;
        let height = natural.round().max(1.0);
        let baseline = (metrics.ascent + (height - natural) / 2.0).round();
        let stroke = metrics.stroke_size.round().max(1.0);
        Self {
            width: advance.round().max(1.0) as u32,
            height: height as u32,
            baseline: baseline as u32,
            px,
            underline: (baseline - metrics.underline_offset).round().clamp(0.0, height - stroke) as u32,
            strikeout: (baseline - metrics.strikeout_offset).round().clamp(0.0, height - stroke) as u32,
            stroke: stroke as u32,
        }
    }
}

fn regular() -> FontRef<'static> {
    Face::Regular.font().expect("the bundled JetBrains Mono parses")
}

pub struct Mask {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub alpha: Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    text: String,
    face: Face,
    px: u32,
    span: u32,
}

pub struct Glyphs {
    context: ScaleContext,
    fonts: Vec<(Face, FontRef<'static>)>,
    masks: HashMap<Key, Option<Arc<Mask>>>,
}

pub static GLYPHS: LazyLock<Mutex<Glyphs>> = LazyLock::new(|| Mutex::new(Glyphs::new()));

const VARIATION_SELECTORS: std::ops::RangeInclusive<char> = '\u{fe00}'..='\u{fe0f}';
const ZERO_WIDTH_JOINER: char = '\u{200d}';

fn is_combining(c: char) -> bool {
    matches!(c, '\u{0300}'..='\u{036f}' | '\u{1ab0}'..='\u{1aff}' | '\u{1dc0}'..='\u{1dff}' | '\u{20d0}'..='\u{20ff}' | '\u{fe20}'..='\u{fe2f}')
}

impl Glyphs {
    fn new() -> Self {
        let fonts = ALL.iter().filter_map(|face| Some((*face, face.font()?))).collect();
        Self { context: ScaleContext::new(), fonts, masks: HashMap::new() }
    }

    fn font(&self, face: Face) -> Option<FontRef<'static>> {
        self.fonts.iter().find(|(f, _)| *f == face).map(|(_, font)| *font)
    }

    pub fn covers(&self, face: Face, c: char) -> bool {
        self.font(face).is_some_and(|font| font.charmap().map(c) != 0)
    }

    pub fn mask(&mut self, text: &str, style: Face, cell: &CellSize, span: u32) -> Option<Arc<Mask>> {
        let key = Key { text: text.to_string(), face: style, px: cell.px.to_bits(), span };
        if let Some(found) = self.masks.get(&key) {
            return found.clone();
        }
        let drawn = self.draw(text, style, cell, span).map(Arc::new);
        self.masks.insert(key, drawn.clone());
        drawn
    }

    fn draw(&mut self, text: &str, style: Face, cell: &CellSize, span: u32) -> Option<Mask> {
        let mut chars = text.chars().filter(|c| !VARIATION_SELECTORS.contains(c));
        let first = chars.next()?;
        if text.chars().count() == 1
            && let Some(mask) = boxes::mask(first, cell.width * span, cell.height, (cell.px / FONT_PX).round() as u8)
        {
            return Some(mask);
        }
        let emoji_first = text.contains('\u{fe0f}');
        let face = self.face_for(first, style, emoji_first)?;
        let mut drawn = self.glyph(face, first, cell, span)?;
        for mark in chars.take_while(|c| *c != ZERO_WIDTH_JOINER).filter(|c| is_combining(*c)) {
            if let Some(mark) = self.face_for(mark, style, false).and_then(|face| self.glyph(face, mark, cell, span)) {
                drawn = merged(drawn, &mark);
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

    fn glyph(&mut self, face: Face, c: char, cell: &CellSize, span: u32) -> Option<Mask> {
        let font = self.font(face)?;
        let id = font.charmap().map(c);
        let room = (cell.width * span) as f32;
        let natural = font.glyph_metrics(&[]).scale(cell.px).advance_width(id);
        let px = if !face.is_text() && natural > room && natural > 0.0 { cell.px * room / natural } else { cell.px };
        let advance = if face.is_text() && span == 1 { room } else { natural * px / cell.px };
        let shift = ((room - advance) / 2.0).max(0.0);
        let mut scaler = self.context.builder(font).size(px).hint(false).build();
        let image = Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .offset(Vector::new(shift.fract(), 0.0))
            .render(&mut scaler, id)?;
        if image.content != Content::Mask {
            return None;
        }
        Some(Mask {
            left: shift.trunc() as i32 + image.placement.left,
            top: cell.baseline as i32 - image.placement.top,
            width: image.placement.width,
            height: image.placement.height,
            alpha: image.data,
        })
    }
}

fn merged(base: Mask, mark: &Mask) -> Mask {
    let left = base.left.min(mark.left);
    let top = base.top.min(mark.top);
    let right = (base.left + base.width as i32).max(mark.left + mark.width as i32);
    let bottom = (base.top + base.height as i32).max(mark.top + mark.height as i32);
    let (width, height) = ((right - left) as u32, (bottom - top) as u32);
    let mut alpha = vec![0u8; (width * height) as usize];
    for part in [&base, mark] {
        for y in 0..part.height {
            for x in 0..part.width {
                let at = ((part.top - top) as u32 + y) * width + (part.left - left) as u32 + x;
                let value = part.alpha[(y * part.width + x) as usize];
                alpha[at as usize] = alpha[at as usize].max(value);
            }
        }
    }
    Mask { left, top, width, height, alpha }
}
