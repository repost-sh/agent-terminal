use anyhow::Result;
use tiny_skia::{Pixmap, PremultipliedColorU8};

use super::Image;
use super::theme::Rgb;

const CHANGED: Rgb = Rgb(0xe6, 0x1e, 0x3c);
const TOLERANCE: u8 = 16;
const FADE: f32 = 0.25;

pub struct Compared {
    pub image: Image,
    pub changed: u64,
    pub pixels: u64,
}

fn color(pixmap: &Pixmap, x: u32, y: u32) -> Option<[u8; 3]> {
    let pixel = pixmap.pixel(x, y)?.demultiply();
    Some([pixel.red(), pixel.green(), pixel.blue()])
}

fn faded([red, green, blue]: [u8; 3]) -> PremultipliedColorU8 {
    let luminance = 0.299 * f32::from(red) + 0.587 * f32::from(green) + 0.114 * f32::from(blue);
    let gray = (255.0 - (255.0 - luminance) * FADE).round() as u8;
    PremultipliedColorU8::from_rgba(gray, gray, gray, 255).expect("an opaque gray")
}

pub fn compare(baseline: &[u8], current: &Image) -> Result<Compared> {
    let baseline = Pixmap::decode_png(baseline)?;
    let now = &current.pixmap;
    let width = baseline.width().max(now.width());
    let height = baseline.height().max(now.height());
    let mut image = Pixmap::new(width, height).expect("a diff of at least one pixel");
    let changed_color = PremultipliedColorU8::from_rgba(CHANGED.0, CHANGED.1, CHANGED.2, 255).expect("an opaque red");
    let mut changed = 0;
    let pixels = image.pixels_mut();
    for y in 0..height {
        for x in 0..width {
            let drawn = match (color(&baseline, x, y), color(now, x, y)) {
                (Some(before), Some(after)) if before.iter().zip(after).all(|(a, b)| a.abs_diff(b) <= TOLERANCE) => {
                    faded(after)
                }
                _ => {
                    changed += 1;
                    changed_color
                }
            };
            pixels[(y * width + x) as usize] = drawn;
        }
    }
    Ok(Compared { image: Image { pixmap: image }, changed, pixels: u64::from(width) * u64::from(height) })
}
