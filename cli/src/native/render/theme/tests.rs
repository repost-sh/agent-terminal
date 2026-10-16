use super::{Rgb, palette};
use crate::native::screen::Colour;
use crate::protocol::Theme;

#[test]
fn a_mix_moves_each_channel_toward_the_other_colour_by_the_amount() {
    let (dark, light) = (Rgb(0, 100, 200), Rgb(100, 100, 0));
    assert_eq!(
        [dark.mix(light, 0.0), dark.mix(light, 1.0), dark.mix(light, 0.25), dark.mix(light, 0.5)],
        [Rgb(0, 100, 200), Rgb(100, 100, 0), Rgb(25, 100, 150), Rgb(50, 100, 100)]
    );
    assert_eq!(Rgb(250, 5, 0).mix(Rgb(255, 0, 9), 2.0), Rgb(255, 0, 18));
}

#[test]
fn a_colour_is_written_as_six_hex_digits() {
    assert_eq!([Rgb(0, 0, 0).css(), Rgb(0x0a, 0xbc, 0xff).css()], ["#000000", "#0abcff"]);
}

#[test]
fn the_sixteen_colours_come_from_the_theme_and_the_rest_from_the_cube_and_gray_ramp() {
    let dark = palette(Theme::Dark);
    let colour = |index| dark.color(Colour::Indexed(index), Rgb(9, 9, 9));
    assert_eq!(
        [colour(0), colour(15), colour(16), colour(17), colour(21), colour(196), colour(231)],
        [dark.ansi[0], dark.ansi[15], Rgb(0, 0, 0), Rgb(0, 0, 95), Rgb(0, 0, 255), Rgb(255, 0, 0), Rgb(255, 255, 255)]
    );
    assert_eq!([colour(232), colour(244), colour(255)], [Rgb(8, 8, 8), Rgb(128, 128, 128), Rgb(238, 238, 238)]);
    assert_eq!(
        [dark.color(Colour::Default, Rgb(9, 9, 9)), dark.color(Colour::Rgb(1, 2, 3), Rgb(9, 9, 9))],
        [Rgb(9, 9, 9), Rgb(1, 2, 3)]
    );
}

#[test]
fn each_theme_has_its_own_ground_and_ref_colours_that_repeat_after_eight() {
    let (dark, light) = (palette(Theme::Dark), palette(Theme::Light));
    assert_eq!(
        (dark.background, dark.foreground, light.background, light.foreground),
        (Rgb(0x11, 0x11, 0x13), Rgb(0xd4, 0xd4, 0xd8), Rgb(0xfa, 0xfa, 0xfa), Rgb(0x27, 0x27, 0x2a))
    );
    assert_eq!(
        [dark.ref_color(0), dark.ref_color(7), dark.ref_color(8), dark.ref_color(9)],
        [dark.refs[0], dark.refs[7], dark.refs[0], dark.refs[1]]
    );
    assert_eq!((light.ansi[1], light.refs[0]), (Rgb(0xdc, 0x26, 0x26), Rgb(0xea, 0x58, 0x0c)));
}
