use std::borrow::Cow;
use std::collections::HashMap;

use anyhow::{Context, Result};
use gif::{DisposalMethod, Encoder, Frame, Repeat};

use super::Movie;
use crate::native::render::Image;

const PALETTE: usize = 256;

#[derive(Default)]
pub struct Gif {
    encoder: Option<Encoder<Vec<u8>>>,
    size: (u32, u32),
    previous: Vec<u8>,
}

struct Rect {
    left: usize,
    top: usize,
    width: usize,
    height: usize,
}

impl Movie for Gif {
    fn add(&mut self, image: &Image, start_ms: u64, end_ms: u64) -> Result<()> {
        let (width, height) = (image.width() as usize, image.height() as usize);
        let rgba = image.rgba();
        let whole = Rect { left: 0, top: 0, width, height };
        let rect = changed(&self.previous, rgba, width, height).unwrap_or(whole);
        let encoder = match &mut self.encoder {
            Some(encoder) => encoder,
            None => {
                let too_large = || format!("a {width}x{height} px picture is too large for a GIF");
                let gif_width = u16::try_from(width).with_context(too_large)?;
                let gif_height = u16::try_from(height).with_context(too_large)?;
                let mut encoder = Encoder::new(Vec::new(), gif_width, gif_height, &[]).context("start the GIF")?;
                encoder.set_repeat(Repeat::Infinite).context("start the GIF")?;
                self.size = (image.width(), image.height());
                self.encoder.insert(encoder)
            }
        };
        let mut delay = centiseconds(end_ms) - centiseconds(start_ms);
        let mut part = rect;
        loop {
            let shown = delay.min(u64::from(u16::MAX));
            let (palette, indices) = indexed(rgba, width, &part);
            let frame = Frame {
                left: part.left as u16,
                top: part.top as u16,
                width: part.width as u16,
                height: part.height as u16,
                buffer: Cow::Owned(indices),
                palette: Some(palette),
                delay: shown as u16,
                dispose: DisposalMethod::Keep,
                ..Frame::default()
            };
            encoder.write_frame(&frame).context("encode a GIF frame")?;
            delay -= shown;
            if delay == 0 {
                break;
            }
            part = Rect { left: part.left, top: part.top, width: 1, height: 1 };
        }
        self.previous.clear();
        self.previous.extend_from_slice(rgba);
        Ok(())
    }

    fn size(&self) -> (u32, u32) {
        self.size
    }

    fn finish(self) -> Result<Vec<u8>> {
        match self.encoder {
            Some(encoder) => encoder.into_inner().context("finish the GIF"),
            None => Ok(Vec::new()),
        }
    }
}

fn centiseconds(ms: u64) -> u64 {
    (ms + 5) / 10
}

fn changed(previous: &[u8], current: &[u8], width: usize, height: usize) -> Option<Rect> {
    if previous.len() != current.len() {
        return None;
    }
    let stride = width * 4;
    let rows: Vec<usize> = (0..height)
        .filter(|y| previous[y * stride..(y + 1) * stride] != current[y * stride..(y + 1) * stride])
        .collect();
    let (&top, &bottom) = (rows.first()?, rows.last()?);
    let differs =
        |x: usize| rows.iter().any(|y| previous[y * stride + x * 4..][..4] != current[y * stride + x * 4..][..4]);
    let left = (0..width).find(|x| differs(*x))?;
    let right = (left..width).rfind(|x| differs(*x))?;
    Some(Rect { left, top, width: right - left + 1, height: bottom - top + 1 })
}

fn indexed(rgba: &[u8], width: usize, rect: &Rect) -> (Vec<u8>, Vec<u8>) {
    let rows = || {
        (rect.top..rect.top + rect.height)
            .map(move |y| &rgba[(y * width + rect.left) * 4..(y * width + rect.left + rect.width) * 4])
    };
    let mut counts: HashMap<[u8; 3], usize> = HashMap::new();
    for row in rows() {
        let mut pixels = row.chunks_exact(4).map(rgb).peekable();
        while let Some(color) = pixels.next() {
            let mut run = 1;
            while pixels.next_if_eq(&color).is_some() {
                run += 1;
            }
            *counts.entry(color).or_default() += run;
        }
    }
    let mut colors: Vec<([u8; 3], usize)> = counts.into_iter().collect();
    colors.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let kept: Vec<[u8; 3]> = colors.iter().take(PALETTE).map(|(color, _)| *color).collect();
    let index: HashMap<[u8; 3], u8> = colors
        .iter()
        .enumerate()
        .map(|(i, (color, _))| (*color, if i < PALETTE { i as u8 } else { nearest(&kept, *color) }))
        .collect();
    let mut indices = Vec::with_capacity(rect.width * rect.height);
    for row in rows() {
        let mut pixels = row.chunks_exact(4).map(rgb).peekable();
        while let Some(color) = pixels.next() {
            let i = index[&color];
            indices.push(i);
            while pixels.next_if_eq(&color).is_some() {
                indices.push(i);
            }
        }
    }
    (kept.concat(), indices)
}

fn rgb(pixel: &[u8]) -> [u8; 3] {
    [pixel[0], pixel[1], pixel[2]]
}

fn nearest(palette: &[[u8; 3]], color: [u8; 3]) -> u8 {
    let distance =
        |entry: &[u8; 3]| -> u32 { entry.iter().zip(color).map(|(a, b)| u32::from(a.abs_diff(b)).pow(2)).sum() };
    let (closest, _) = palette.iter().enumerate().fold((0, u32::MAX), |best, (i, entry)| {
        let away = distance(entry);
        if away < best.1 { (i, away) } else { best }
    });
    closest as u8
}
