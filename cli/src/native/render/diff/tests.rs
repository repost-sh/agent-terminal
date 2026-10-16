use tiny_skia::{Color, Pixmap};

use super::super::fonts::CellSize;
use super::super::{Cells, Image, PAD};
use super::{CellBox, compare, faded, runs, same};
use crate::protocol::RowRange;

fn painted(width: u32, height: u32, marks: &[(u32, u32, [u8; 3])]) -> Image {
    let mut pixmap = Pixmap::new(width, height).unwrap();
    pixmap.fill(Color::from_rgba8(0, 0, 0, 255));
    for &(x, y, [r, g, b]) in marks {
        let at = usize::try_from((y * width + x) * 4).unwrap();
        pixmap.data_mut()[at..at + 4].copy_from_slice(&[r, g, b, 255]);
    }
    Image { pixmap }
}

fn rgb(image: &Image, x: u32, y: u32) -> [u8; 3] {
    let pixel = image.pixmap.pixel(x, y).unwrap().demultiply();
    [pixel.red(), pixel.green(), pixel.blue()]
}

fn spans(cols: u32, differs: impl Fn(u32) -> bool) -> Vec<(u32, u32)> {
    runs(cols, differs).into_iter().map(|run| (run.start, run.end)).collect()
}

#[test]
fn runs_group_neighbouring_changed_columns_and_close_at_the_last_column() {
    assert_eq!(spans(6, |col| [1, 2, 4, 5].contains(&col)), [(1, 3), (4, 6)]);
    assert_eq!(spans(3, |_| true), [(0, 3)]);
    assert!(spans(3, |_| false).is_empty());
    assert_eq!(spans(1, |col| col == 0), [(0, 1)]);
}

#[test]
fn colours_within_sixteen_per_channel_are_the_same_and_a_missing_pixel_never_is() {
    assert!(same(Some([100, 100, 100]), Some([116, 84, 100])));
    assert!(!same(Some([100, 100, 100]), Some([117, 100, 100])));
    assert!(!same(Some([100, 100, 100]), Some([100, 83, 100])));
    assert!(!same(None, Some([0, 0, 0])));
    assert!(!same(Some([0, 0, 0]), None));
}

#[test]
fn an_unchanged_pixel_fades_to_a_light_gray_of_its_luminance() {
    let gray = |colour| {
        let pixel = faded(colour).unwrap();
        [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]
    };
    assert_eq!(gray([255, 255, 255]), [255, 255, 255, 255]);
    assert_eq!(gray([0, 0, 0]), [191, 191, 191, 255]);
    assert_eq!(gray([255, 0, 0]), [210, 210, 210, 255]);
    assert_eq!(gray([0, 255, 0]), [229, 229, 229, 255]);
    assert_eq!(gray([0, 0, 255]), [199, 199, 199, 255]);
}

#[test]
fn a_comparison_marks_changed_pixels_red_over_the_larger_size_and_fades_the_rest() {
    let baseline = painted(3, 2, &[(1, 0, [200, 200, 200])]).png().unwrap();
    let compared = compare(&baseline, &painted(4, 2, &[(1, 0, [210, 190, 200]), (2, 1, [90, 0, 0])])).unwrap();
    let (gray, light, red) = ([191, 191, 191], [241, 241, 241], [0xe6, 0x1e, 0x3c]);
    let pixels: Vec<[u8; 3]> =
        (0..2).flat_map(|y| (0..4).map(move |x| (x, y))).map(|(x, y)| rgb(&compared.image, x, y)).collect();
    assert_eq!((compared.changed, compared.pixels, compared.image.width(), compared.image.height()), (3, 8, 4, 2));
    assert_eq!(pixels, [gray, light, gray, red, gray, gray, red, red]);
}

#[test]
fn changed_cells_count_each_cell_with_a_changed_pixel_and_box_each_run_on_its_row() {
    let cell = CellSize::at(1).unwrap();
    let (width, height) = (u32::from(cell.width), u32::from(cell.height));
    let size = (PAD * 2 + width * 5, PAD * 2 + height * 3);
    let at = |col: u32, row: u32| (PAD + col * width + width - 1, PAD + row * height + height - 1, [255, 255, 255]);
    let older = painted(size.0, size.1, &[]).png().unwrap();
    let newer = painted(size.0, size.1, &[at(1, 0), at(2, 0), at(4, 0), at(0, 2)]).png().unwrap();
    let cells = Cells { scale: 1, cols: 5, rows: RowRange { first: 4, last: 6 } };
    let changed = cells.changed_between(&older, &newer).unwrap();
    assert_eq!(changed.count, 4);
    assert_eq!(
        changed.boxes,
        [
            CellBox { x: PAD + width, y: PAD, width: 2 * width, height },
            CellBox { x: PAD + 4 * width, y: PAD, width, height },
            CellBox { x: PAD, y: PAD + 2 * height, width, height },
        ]
    );
}

#[test]
fn changed_cells_at_scale_two_use_the_wider_cells_and_padding() {
    let cell = CellSize::at(2).unwrap();
    let (width, height, pad) = (u32::from(cell.width), u32::from(cell.height), PAD * 2);
    let size = (pad * 2 + width * 2, pad * 2 + height);
    let older = painted(size.0, size.1, &[]).png().unwrap();
    let newer = painted(size.0, size.1, &[(pad + width, pad, [255, 255, 255])]).png().unwrap();
    let cells = Cells { scale: 2, cols: 2, rows: RowRange { first: 1, last: 1 } };
    let changed = cells.changed_between(&older, &newer).unwrap();
    assert_eq!((changed.count, changed.boxes), (1, vec![CellBox { x: pad + width, y: pad, width, height }]));
}
