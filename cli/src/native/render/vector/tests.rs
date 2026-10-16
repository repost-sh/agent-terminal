use super::super::Drawing;
use super::{html, svg};
use crate::native::refs::{Decoration, Emphasis, Glyph, Grid};
use crate::native::screen::Colour;
use crate::protocol::{Look, Ref, RefKind, RowRange, Theme};

fn glyph(text: &str) -> Glyph {
    Glyph {
        text: text.to_string(),
        fg: Colour::Default,
        bg: Colour::Default,
        emphasis: Emphasis { bold: false, italic: false, dim: false },
        decoration: Decoration { underline: false, strikeout: false },
        inverse: false,
        wide: false,
        link: None,
    }
}

fn styled_screen() -> Grid {
    let mut first: Vec<Glyph> = "ab<&>\"".chars().map(|c| glyph(&c.to_string())).collect();
    first[1].emphasis.bold = true;
    first[2].emphasis.italic = true;
    first[3].decoration.underline = true;
    first[4].decoration = Decoration { underline: true, strikeout: true };
    first[5].fg = Colour::Indexed(2);
    let mut second: Vec<Glyph> = ["中", "", "x", "\u{7}", " ", " "].iter().map(|text| glyph(text)).collect();
    second[0].wide = true;
    second[2].bg = Colour::Rgb(1, 2, 3);
    second[3].fg = Colour::Indexed(196);
    let mut third: Vec<Glyph> = "okay  ".chars().map(|c| glyph(&c.to_string())).collect();
    third[5].decoration.strikeout = true;
    Grid { rows: vec![first, second, third], cursor: Some((2, 0)) }
}

fn button(id: &str, rows: [u16; 2], cols: [u16; 2]) -> Ref {
    Ref {
        id: id.to_string(),
        kind: RefKind::Button,
        label: "<ok>".to_string(),
        row: rows[0],
        col: cols[0],
        rows,
        cols,
        frame: 1,
        url: None,
    }
}

fn drawing(screen: &Grid, look: Look) -> Drawing<'_> {
    Drawing { grid: screen, rows: RowRange { first: 1, last: 3 }, look }
}

#[test]
fn an_svg_draws_each_run_of_one_style_with_its_backgrounds_refs_and_legend() {
    let screen = styled_screen();
    let look = Look { scale: 1, theme: Theme::Dark, cursor: true };
    let drawn = svg(&drawing(&screen, look), &[button("1", [1, 1], [1, 2]), button("2&", [3, 3], [1, 4])]).unwrap();
    assert_eq!((drawn.width, drawn.height), (200, 130));
    insta::assert_snapshot!(drawn.text, @r##"
    <svg xmlns="http://www.w3.org/2000/svg" width="200" height="130" viewBox="0 0 200 130" font-family="'JetBrains Mono', 'JetBrainsMono Nerd Font', ui-monospace, Menlo, Consolas, monospace" font-size="15">
    <rect width="100%" height="100%" fill="#111113"/>
    <text x="10" y="25" textLength="9" lengthAdjust="spacingAndGlyphs" xml:space="preserve" fill="#d4d4d8">a</text>
    <text x="19" y="25" textLength="9" lengthAdjust="spacingAndGlyphs" xml:space="preserve" fill="#d4d4d8" font-weight="bold">b</text>
    <text x="28" y="25" textLength="9" lengthAdjust="spacingAndGlyphs" xml:space="preserve" fill="#d4d4d8" font-style="italic">&lt;</text>
    <text x="37" y="25" textLength="9" lengthAdjust="spacingAndGlyphs" xml:space="preserve" fill="#d4d4d8" text-decoration="underline">&amp;</text>
    <text x="46" y="25" textLength="9" lengthAdjust="spacingAndGlyphs" xml:space="preserve" fill="#d4d4d8" text-decoration="underline line-through">&gt;</text>
    <text x="55" y="25" textLength="9" lengthAdjust="spacingAndGlyphs" xml:space="preserve" fill="#4ade80">&quot;</text>
    <text x="10" y="45" textLength="18" lengthAdjust="spacingAndGlyphs" xml:space="preserve" fill="#d4d4d8">中</text>
    <rect x="28" y="30" width="9" height="20" fill="#010203"/>
    <text x="28" y="45" textLength="9" lengthAdjust="spacingAndGlyphs" xml:space="preserve" fill="#d4d4d8">x</text>
    <text x="37" y="45" textLength="9" lengthAdjust="spacingAndGlyphs" xml:space="preserve" fill="#ff0000"> </text>
    <rect x="10" y="50" width="9" height="20" fill="#d4d4d8"/>
    <text x="10" y="65" textLength="9" lengthAdjust="spacingAndGlyphs" xml:space="preserve" fill="#111113">o</text>
    <text x="19" y="65" textLength="36" lengthAdjust="spacingAndGlyphs" xml:space="preserve" fill="#d4d4d8">kay </text>
    <text x="55" y="65" textLength="9" lengthAdjust="spacingAndGlyphs" xml:space="preserve" fill="#d4d4d8" text-decoration="line-through"> </text>
    <rect x="11" y="11" width="16" height="18" fill="none" stroke="#fb923c" stroke-width="2"/>
    <rect x="18" y="10" width="10" height="14" fill="#fb923c"/><text x="20" y="21" font-size="10.5" fill="#111113">1</text>
    <rect x="11" y="51" width="34" height="18" fill="none" stroke="#38bdf8" stroke-width="2"/>
    <rect x="30" y="36" width="16" height="14" fill="#38bdf8"/><text x="32" y="47" font-size="10.5" fill="#111113">2&amp;</text>
    <rect x="10" y="80" width="9" height="20" fill="#fb923c"/><text x="10" y="95" fill="#111113">1</text><text x="28" y="95" fill="#d4d4d8" xml:space="preserve">button &quot;&lt;ok&gt;&quot;</text>
    <rect x="10" y="100" width="18" height="20" fill="#38bdf8"/><text x="10" y="115" fill="#111113">2&amp;</text><text x="37" y="115" fill="#d4d4d8" xml:space="preserve">button &quot;&lt;ok&gt;&quot;</text>
    </svg>
    "##);
}

#[test]
fn an_html_page_styles_only_what_differs_from_the_theme_and_trims_blank_row_ends() {
    let screen = styled_screen();
    insta::assert_snapshot!(html(&drawing(&screen, Look { scale: 1, theme: Theme::Dark, cursor: true })).unwrap(), @r##"
    <!DOCTYPE html>
    <html>
    <head>
    <meta charset="utf-8">
    <title>agent-terminal screenshot</title>
    <style>
    body { margin: 0; background: #111113; }
    pre { margin: 0; padding: 10px; color: #d4d4d8; background: #111113; font: 15px/20px 'JetBrains Mono', 'JetBrainsMono Nerd Font', ui-monospace, Menlo, Consolas, monospace; }
    </style>
    </head>
    <body>
    <pre>a<span style="font-weight:bold">b</span><span style="font-style:italic">&lt;</span><span style="text-decoration:underline">&amp;</span><span style="text-decoration:underline line-through">&gt;</span><span style="color:#4ade80">&quot;</span>
    中<span style="background:#010203">x</span><span style="color:#ff0000"> </span>
    <span style="color:#111113;background:#d4d4d8">o</span>kay <span style="text-decoration:line-through"> </span>
    </pre>
    </body>
    </html>
    "##);
}

#[test]
fn an_html_page_takes_its_size_and_colours_from_the_look() {
    let screen = styled_screen();
    insta::assert_snapshot!(html(&drawing(&screen, Look { scale: 2, theme: Theme::Light, cursor: false })).unwrap(), @r##"
    <!DOCTYPE html>
    <html>
    <head>
    <meta charset="utf-8">
    <title>agent-terminal screenshot</title>
    <style>
    body { margin: 0; background: #fafafa; }
    pre { margin: 0; padding: 20px; color: #27272a; background: #fafafa; font: 30px/40px 'JetBrains Mono', 'JetBrainsMono Nerd Font', ui-monospace, Menlo, Consolas, monospace; }
    </style>
    </head>
    <body>
    <pre>a<span style="font-weight:bold">b</span><span style="font-style:italic">&lt;</span><span style="text-decoration:underline">&amp;</span><span style="text-decoration:underline line-through">&gt;</span><span style="color:#16a34a">&quot;</span>
    中<span style="background:#010203">x</span><span style="color:#ff0000"> </span>
    okay <span style="text-decoration:line-through"> </span>
    </pre>
    </body>
    </html>
    "##);
}
