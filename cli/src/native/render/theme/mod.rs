use num_traits::ToPrimitive;

use crate::native::screen::Colour;
use crate::protocol::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub(super) fn mix(self, other: Rgb, amount: f32) -> Rgb {
        let blend = |a: u8, b: u8| {
            let (a, b) = (f32::from(a), f32::from(b));
            (a + (b - a) * amount).round().clamp(0.0, 255.0).to_u8().unwrap_or_default()
        };
        Rgb(blend(self.0, other.0), blend(self.1, other.1), blend(self.2, other.2))
    }

    pub(super) fn css(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

pub(super) struct Palette {
    pub background: Rgb,
    pub foreground: Rgb,
    pub ansi: [Rgb; 16],
    pub refs: [Rgb; 8],
}

const DARK: Palette = Palette {
    background: Rgb(0x11, 0x11, 0x13),
    foreground: Rgb(0xd4, 0xd4, 0xd8),
    ansi: [
        Rgb(0x27, 0x27, 0x2a),
        Rgb(0xf8, 0x71, 0x71),
        Rgb(0x4a, 0xde, 0x80),
        Rgb(0xfa, 0xcc, 0x15),
        Rgb(0x60, 0xa5, 0xfa),
        Rgb(0xc0, 0x84, 0xfc),
        Rgb(0x22, 0xd3, 0xee),
        Rgb(0xd4, 0xd4, 0xd8),
        Rgb(0x71, 0x71, 0x7a),
        Rgb(0xfc, 0xa5, 0xa5),
        Rgb(0x86, 0xef, 0xac),
        Rgb(0xfd, 0xe0, 0x47),
        Rgb(0x93, 0xc5, 0xfd),
        Rgb(0xd8, 0xb4, 0xfe),
        Rgb(0x67, 0xe8, 0xf9),
        Rgb(0xfa, 0xfa, 0xfa),
    ],
    refs: [
        Rgb(0xfb, 0x92, 0x3c),
        Rgb(0x38, 0xbd, 0xf8),
        Rgb(0xa3, 0xe6, 0x35),
        Rgb(0xf4, 0x72, 0xb6),
        Rgb(0xfa, 0xcc, 0x15),
        Rgb(0x81, 0x8c, 0xf8),
        Rgb(0x2d, 0xd4, 0xbf),
        Rgb(0xf8, 0x71, 0x71),
    ],
};

const LIGHT: Palette = Palette {
    background: Rgb(0xfa, 0xfa, 0xfa),
    foreground: Rgb(0x27, 0x27, 0x2a),
    ansi: [
        Rgb(0x27, 0x27, 0x2a),
        Rgb(0xdc, 0x26, 0x26),
        Rgb(0x16, 0xa3, 0x4a),
        Rgb(0xa1, 0x62, 0x07),
        Rgb(0x25, 0x63, 0xeb),
        Rgb(0x93, 0x33, 0xea),
        Rgb(0x0e, 0x74, 0x90),
        Rgb(0xa1, 0xa1, 0xaa),
        Rgb(0x52, 0x52, 0x5b),
        Rgb(0xef, 0x44, 0x44),
        Rgb(0x22, 0xc5, 0x5e),
        Rgb(0xca, 0x8a, 0x04),
        Rgb(0x3b, 0x82, 0xf6),
        Rgb(0xa8, 0x55, 0xf7),
        Rgb(0x08, 0x91, 0xb2),
        Rgb(0xd4, 0xd4, 0xd8),
    ],
    refs: [
        Rgb(0xea, 0x58, 0x0c),
        Rgb(0x02, 0x84, 0xc7),
        Rgb(0x65, 0xa3, 0x0d),
        Rgb(0xdb, 0x27, 0x77),
        Rgb(0xca, 0x8a, 0x04),
        Rgb(0x4f, 0x46, 0xe5),
        Rgb(0x0d, 0x94, 0x88),
        Rgb(0xdc, 0x26, 0x26),
    ],
};

pub(super) fn palette(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Dark => &DARK,
        Theme::Light => &LIGHT,
    }
}

impl Palette {
    pub(super) fn color(&self, colour: Colour, default: Rgb) -> Rgb {
        match colour {
            Colour::Default => default,
            Colour::Indexed(index) => self.indexed(index),
            Colour::Rgb(red, green, blue) => Rgb(red, green, blue),
        }
    }

    fn indexed(&self, index: u8) -> Rgb {
        match index {
            0..=15 => self.ansi.get(usize::from(index)).copied().unwrap_or(self.foreground),
            16..=231 => {
                let cube = index - 16;
                let level = |step: u8| if step == 0 { 0 } else { 55 + step * 40 };
                Rgb(level(cube / 36), level(cube / 6 % 6), level(cube % 6))
            }
            _ => {
                let gray = 8 + (index - 232) * 10;
                Rgb(gray, gray, gray)
            }
        }
    }

    pub(super) fn ref_color(&self, index: usize) -> Rgb {
        let [first, ..] = self.refs;
        self.refs.get(index % self.refs.len()).copied().unwrap_or(first)
    }
}

#[cfg(test)]
mod tests;
