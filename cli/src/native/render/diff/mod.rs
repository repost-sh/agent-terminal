use std::ops::Range;

use anyhow::{Context, Result};
use num_traits::ToPrimitive;
use serde::Serialize;
use tiny_skia::{Pixmap, PremultipliedColorU8};

use super::fonts::CellSize;
use super::theme::Rgb;
use super::{Cells, Image, PAD};

const CHANGED: Rgb = Rgb(0xe6, 0x1e, 0x3c);
const TOLERANCE: u8 = 16;
const FADE: f32 = 0.25;

pub(crate) struct Compared {
    pub image: Image,
    pub changed: u32,
    pub pixels: u32,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub(crate) struct CellBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub(crate) struct ChangedCells {
    pub count: u32,
    pub boxes: Vec<CellBox>,
}

fn color(pixmap: &Pixmap, x: u32, y: u32) -> Option<[u8; 3]> {
    let pixel = pixmap.pixel(x, y).filter(|_| x < pixmap.width())?.demultiply();
    Some([pixel.red(), pixel.green(), pixel.blue()])
}

fn faded([red, green, blue]: [u8; 3]) -> Option<PremultipliedColorU8> {
    let luminance = 0.299 * f32::from(red) + 0.587 * f32::from(green) + 0.114 * f32::from(blue);
    let gray = (255.0 - (255.0 - luminance) * FADE).round().clamp(0.0, 255.0).to_u8().unwrap_or_default();
    PremultipliedColorU8::from_rgba(gray, gray, gray, 255)
}

fn same(a: Option<[u8; 3]>, b: Option<[u8; 3]>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.iter().zip(b).all(|(a, b)| a.abs_diff(b) <= TOLERANCE),
        _ => false,
    }
}

impl Cells {
    pub(crate) fn changed_between(self, older: &[u8], newer: &[u8]) -> Result<ChangedCells> {
        let (older, newer) = (Pixmap::decode_png(older)?, Pixmap::decode_png(newer)?);
        let cell = CellSize::at(self.scale)?;
        let (width, height) = (u32::from(cell.width), u32::from(cell.height));
        let pad = PAD * u32::from(self.scale.max(1));
        let rows = u32::from(self.rows.last.saturating_sub(self.rows.first)) + 1;
        let differs = |col: u32, row: u32| {
            let (left, top) = (pad + col * width, pad + row * height);
            (top..top + height).any(|y| (left..left + width).any(|x| !same(color(&older, x, y), color(&newer, x, y))))
        };
        let mut changed = ChangedCells { count: 0, boxes: Vec::new() };
        for row in 0..rows {
            for run in runs(u32::from(self.cols), |col| differs(col, row)) {
                changed.count += run.end - run.start;
                changed.boxes.push(CellBox {
                    x: pad + run.start * width,
                    y: pad + row * height,
                    width: (run.end - run.start) * width,
                    height,
                });
            }
        }
        Ok(changed)
    }
}

fn runs(cols: u32, differs: impl Fn(u32) -> bool) -> Vec<Range<u32>> {
    let mut found = Vec::new();
    let mut run: Option<u32> = None;
    for col in 0..=cols {
        match (run, col < cols && differs(col)) {
            (None, true) => run = Some(col),
            (Some(first), false) => {
                found.push(first..col);
                run = None;
            }
            _ => {}
        }
    }
    found
}

pub(crate) fn compare(baseline: &[u8], current: &Image) -> Result<Compared> {
    let baseline = Pixmap::decode_png(baseline)?;
    let now = &current.pixmap;
    let width = baseline.width().max(now.width());
    let height = baseline.height().max(now.height());
    let pixels = u32::try_from(u64::from(width) * u64::from(height)).context("the images are too large to compare")?;
    let mut image = Pixmap::new(width, height).context("a diff needs at least one pixel")?;
    let changed_color =
        PremultipliedColorU8::from_rgba(CHANGED.0, CHANGED.1, CHANGED.2, 255).context("an opaque red")?;
    let mut changed = 0;
    let stride = usize::try_from(width)?;
    for (y, row) in (0..height).zip(image.pixels_mut().chunks_mut(stride)) {
        for (x, pixel) in (0..width).zip(row) {
            let (before, after) = (color(&baseline, x, y), color(now, x, y));
            *pixel = if let Some(faded) = after.filter(|&after| same(before, Some(after))).and_then(faded) {
                faded
            } else {
                changed += 1;
                changed_color
            };
        }
    }
    Ok(Compared { image: Image { pixmap: image }, changed, pixels })
}

#[cfg(test)]
mod tests;
