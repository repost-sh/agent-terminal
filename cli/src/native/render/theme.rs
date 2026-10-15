use crate::native::screen::Colour;
use crate::protocol::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn mix(self, other: Rgb, amount: f32) -> Rgb {
        let blend = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
        Rgb(blend(self.0, other.0), blend(self.1, other.1), blend(self.2, other.2))
    }

    pub fn css(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

pub struct Palette {
    pub background: Rgb,
    pub foreground: Rgb,
    ansi: [Rgb; 16],
    pub refs: [Rgb; 8],
}

const fn rgb(value: u32) -> Rgb {
    Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

const DARK: Palette = Palette {
    background: rgb(0x111113),
    foreground: rgb(0xd4d4d8),
    ansi: [
        rgb(0x27272a),
        rgb(0xf87171),
        rgb(0x4ade80),
        rgb(0xfacc15),
        rgb(0x60a5fa),
        rgb(0xc084fc),
        rgb(0x22d3ee),
        rgb(0xd4d4d8),
        rgb(0x71717a),
        rgb(0xfca5a5),
        rgb(0x86efac),
        rgb(0xfde047),
        rgb(0x93c5fd),
        rgb(0xd8b4fe),
        rgb(0x67e8f9),
        rgb(0xfafafa),
    ],
    refs: [
        rgb(0xfb923c),
        rgb(0x38bdf8),
        rgb(0xa3e635),
        rgb(0xf472b6),
        rgb(0xfacc15),
        rgb(0x818cf8),
        rgb(0x2dd4bf),
        rgb(0xf87171),
    ],
};

const LIGHT: Palette = Palette {
    background: rgb(0xfafafa),
    foreground: rgb(0x27272a),
    ansi: [
        rgb(0x27272a),
        rgb(0xdc2626),
        rgb(0x16a34a),
        rgb(0xa16207),
        rgb(0x2563eb),
        rgb(0x9333ea),
        rgb(0x0e7490),
        rgb(0xa1a1aa),
        rgb(0x52525b),
        rgb(0xef4444),
        rgb(0x22c55e),
        rgb(0xca8a04),
        rgb(0x3b82f6),
        rgb(0xa855f7),
        rgb(0x0891b2),
        rgb(0xd4d4d8),
    ],
    refs: [
        rgb(0xea580c),
        rgb(0x0284c7),
        rgb(0x65a30d),
        rgb(0xdb2777),
        rgb(0xca8a04),
        rgb(0x4f46e5),
        rgb(0x0d9488),
        rgb(0xdc2626),
    ],
};

pub fn palette(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Dark => &DARK,
        Theme::Light => &LIGHT,
    }
}

impl Palette {
    pub fn color(&self, colour: Colour, default: Rgb) -> Rgb {
        match colour {
            Colour::Default => default,
            Colour::Indexed(index) => self.indexed(index),
            Colour::Rgb(red, green, blue) => Rgb(red, green, blue),
        }
    }

    fn indexed(&self, index: u8) -> Rgb {
        match index {
            0..=15 => self.ansi[index as usize],
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

    pub fn ref_color(&self, index: usize) -> Rgb {
        self.refs[index % self.refs.len()]
    }
}
