use super::{CellSize, Face, GLYPHS, Letters};

fn placed(text: &str) -> (i32, i32, u32, u32) {
    let cell = CellSize::at(1).unwrap();
    let mask = GLYPHS.lock().unwrap().mask(&Letters { text, face: Face::Regular, span: 1 }, &cell).unwrap();
    (mask.left, mask.top, mask.width, mask.height)
}

#[test]
fn a_combining_mark_is_drawn_over_its_letter_inside_the_cell() {
    assert_eq!([placed("e"), placed("e\u{301}"), placed("a\u{308}")], [(1, 6, 7, 10), (1, 3, 7, 13), (1, 3, 7, 13)]);
}

use std::sync::Arc;

use super::{Mask, fitted_px, merged, paint};

fn mask(left: i32, top: i32, width: u32, alpha: &[u8]) -> Mask {
    let height = u32::try_from(alpha.len()).unwrap() / width.max(1);
    Mask { left, top, width, height, alpha: alpha.to_vec() }
}

#[test]
fn each_style_has_its_own_face_and_only_the_four_text_faces_are_text() {
    let styles =
        [Face::styled(false, false), Face::styled(true, false), Face::styled(false, true), Face::styled(true, true)];
    assert_eq!(styles, [Face::Regular, Face::Bold, Face::Italic, Face::BoldItalic]);
    assert_eq!(Face::ALL.map(Face::is_text), [true, true, true, true, false, false]);
    assert_eq!(
        Face::ALL.map(Face::name),
        [
            "JetBrains Mono",
            "JetBrains Mono Bold",
            "JetBrains Mono Italic",
            "JetBrains Mono Bold Italic",
            "Symbols Nerd Font Mono",
            "Noto Emoji"
        ]
    );
    assert_eq!(
        Face::ALL.map(Face::file_name),
        [
            "JetBrainsMonoNL-Regular.ttf",
            "JetBrainsMonoNL-Bold.ttf",
            "JetBrainsMonoNL-Italic.ttf",
            "JetBrainsMonoNL-BoldItalic.ttf",
            "SymbolsNerdFontMono-Regular.ttf",
            "NotoEmoji-VariableFont_wght.ttf"
        ]
    );
    assert!(Face::ALL.iter().all(|face| face.font().is_some() && face.bytes().len() > 1000));
}

#[test]
fn a_cell_is_sized_from_the_bundled_font_at_fifteen_pixels_a_scale_step() {
    let sized = |scale| {
        let cell = CellSize::at(scale).unwrap();
        (cell.width, cell.height, cell.baseline, cell.underline, cell.strikeout, cell.stroke, cell.px)
    };
    assert_eq!(
        [sized(0), sized(1), sized(2), sized(3)],
        [
            (9, 20, 15, 17, 10, 1, 15.0),
            (9, 20, 15, 17, 10, 1, 15.0),
            (18, 40, 31, 36, 21, 2, 30.0),
            (27, 59, 46, 53, 32, 2, 45.0)
        ]
    );
}

#[test]
fn a_mask_covering_two_others_spans_both_and_takes_the_stronger_coverage() {
    let base = mask(1, 2, 2, &[10, 20, 30, 40]);
    let mark = mask(0, 1, 2, &[50, 5, 6, 70]);
    let both = merged(&base, &mark).unwrap();
    assert_eq!((both.left, both.top, both.width, both.height), (0, 1, 3, 3));
    assert_eq!(both.alpha, [50, 5, 0, 6, 70, 20, 0, 30, 40]);
    assert_eq!(both.rows().map(<[u8]>::len).collect::<Vec<_>>(), [3, 3, 3]);
    assert_eq!(mask(0, 0, 0, &[]).rows().count(), 0);
}

#[test]
fn painting_outside_a_mask_fails_instead_of_wrapping() {
    let part = mask(0, 0, 2, &[1, 2]);
    let mut alpha = vec![0u8; 4];
    assert_eq!(paint(&mut alpha, 2, (0, 1), &part), Some(()));
    assert_eq!(alpha, [0, 0, 1, 2]);
    assert_eq!(paint(&mut alpha, 2, (1, 1), &part), None);
    assert_eq!(paint(&mut alpha, 2, (0, 2), &part), None);
}

#[test]
fn only_a_symbol_or_emoji_wider_than_its_room_is_shrunk_to_fit() {
    assert_eq!(
        [
            fitted_px(Face::Symbols, 20.0, 10.0, 15.0),
            fitted_px(Face::Emoji, 30.0, 10.0, 15.0),
            fitted_px(Face::Symbols, 10.0, 10.0, 15.0),
            fitted_px(Face::Symbols, 0.0, 0.0, 15.0),
            fitted_px(Face::Regular, 20.0, 10.0, 15.0),
        ]
        .map(f32::to_bits),
        [7.5_f32, 5.0, 15.0, 15.0, 15.0].map(f32::to_bits)
    );
}

fn drawn(text: &str, face: Face, span: u16) -> Option<(i32, i32, u32, u32, u32)> {
    let cell = CellSize::at(1).unwrap();
    let mask = GLYPHS.lock().unwrap().mask(&Letters { text, face, span }, &cell)?;
    Some((mask.left, mask.top, mask.width, mask.height, mask.alpha.iter().map(|&alpha| u32::from(alpha)).sum()))
}

#[test]
fn a_letter_is_drawn_in_its_style_at_its_place_in_the_cell() {
    assert_eq!(
        [drawn("M", Face::Regular, 1), drawn("M", Face::Bold, 1), drawn("g", Face::Regular, 1)],
        [Some((1, 4, 7, 11, 10088)), Some((0, 4, 9, 11, 12932)), Some((1, 6, 7, 12, 8915))]
    );
}

#[test]
fn a_letter_the_style_lacks_falls_back_to_the_symbol_and_emoji_faces_or_is_not_drawn() {
    assert_eq!(
        [
            drawn("\u{e0b0}", Face::Regular, 1),
            drawn("\u{1f600}", Face::Regular, 2),
            drawn("\u{1f600}", Face::Regular, 1),
            drawn("中", Face::Regular, 2),
            drawn("", Face::Regular, 1),
        ],
        [Some((-1, -1, 10, 21, 26116)), Some((1, 3, 16, 15, 11980)), Some((0, 9, 9, 8, 2998)), None, None]
    );
}

#[test]
fn an_emoji_presentation_selector_prefers_the_emoji_face_and_is_not_drawn_itself() {
    assert_eq!(drawn("\u{2764}\u{fe0f}", Face::Regular, 2), Some((1, 2, 16, 16, 16393)));
    assert_eq!(drawn("\u{2764}", Face::Regular, 1), Some((0, 8, 9, 9, 4051)));
    assert_eq!(drawn("M\u{fe0e}", Face::Regular, 1), drawn("M", Face::Regular, 1));
}

#[test]
fn a_box_drawing_letter_alone_is_drawn_across_its_span_and_a_joiner_ends_the_marks() {
    assert_eq!(drawn("─", Face::Regular, 1), Some((0, 0, 9, 20, 2295)));
    assert_eq!(drawn("─", Face::Regular, 2), Some((0, 0, 18, 20, 4590)));
    assert_eq!(drawn("a\u{200d}\u{301}", Face::Regular, 1), drawn("a", Face::Regular, 1));
    assert_ne!(drawn("─\u{301}", Face::Regular, 1), drawn("─", Face::Regular, 1));
}

#[test]
fn a_drawn_mask_is_kept_and_shared_for_the_same_letters_size_and_span() {
    let cell = CellSize::at(1).unwrap();
    let mut glyphs = GLYPHS.lock().unwrap();
    let letters = Letters { text: "Q", face: Face::Italic, span: 1 };
    let first = glyphs.mask(&letters, &cell).unwrap();
    assert!(Arc::ptr_eq(&first, &glyphs.mask(&letters, &cell).unwrap()));
    let wider = glyphs.mask(&Letters { span: 2, ..letters }, &cell).unwrap();
    assert!(!Arc::ptr_eq(&first, &wider));
    assert!(
        glyphs.covers(Face::Regular, 'M')
            && !glyphs.covers(Face::Emoji, 'M')
            && glyphs.covers(Face::Symbols, '\u{e0b0}')
    );
}

#[test]
fn only_marks_are_drawn_over_a_letter_and_a_narrow_letter_is_centred_across_two_cells() {
    assert_eq!(drawn("ab", Face::Regular, 1), drawn("a", Face::Regular, 1));
    assert_eq!(drawn("M", Face::Regular, 2), Some((5, 4, 8, 11, 10087)));
}

#[test]
fn a_box_drawing_letter_is_drawn_at_the_cells_scale() {
    let cell = CellSize::at(2).unwrap();
    let mask = GLYPHS.lock().unwrap().mask(&Letters { text: "─", face: Face::Regular, span: 1 }, &cell).unwrap();
    let boxed = super::boxes::mask('─', &super::BoxCell { width: cell.width, height: cell.height, scale: 2 }).unwrap();
    assert_eq!((mask.width, mask.height, &mask.alpha), (boxed.width, boxed.height, &boxed.alpha));
}
