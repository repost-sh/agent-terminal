use std::fmt;

use alacritty_terminal::vte::ansi::{Color, NamedColor};

const COLOR_NAMES: [&str; 16] = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright-black",
    "bright-red",
    "bright-green",
    "bright-yellow",
    "bright-blue",
    "bright-magenta",
    "bright-cyan",
    "bright-white",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Colour {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl From<Color> for Colour {
    fn from(color: Color) -> Self {
        match color {
            Color::Spec(rgb) => Colour::Rgb(rgb.r, rgb.g, rgb.b),
            Color::Indexed(index) => Colour::Indexed(index),
            Color::Named(named) => {
                let (index, dim_black) = (named as u16, NamedColor::DimBlack as u16);
                let indexed = match index {
                    0..=15 => u8::try_from(index).ok(),
                    _ if (dim_black..dim_black + 8).contains(&index) => u8::try_from(index - dim_black).ok(),
                    _ => None,
                };
                indexed.map_or(Colour::Default, Colour::Indexed)
            }
        }
    }
}

impl fmt::Display for Colour {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Colour::Default => f.write_str("default"),
            Colour::Indexed(index) => match COLOR_NAMES.get(usize::from(*index)) {
                Some(name) => f.write_str(name),
                None => write!(f, "{index}"),
            },
            Colour::Rgb(r, g, b) => write!(f, "#{r:02x}{g:02x}{b:02x}"),
        }
    }
}
