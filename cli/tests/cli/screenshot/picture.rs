use std::path::Path;

pub(super) struct Picture {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) rgb: Vec<u8>,
}

pub(super) fn picture(path: &Path) -> Picture {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().unwrap_or_else(|e| panic!("{} is not a PNG: {e}", path.display()));
    let mut pixels = vec![0; reader.output_buffer_size().expect("a decodable size")];
    let frame = reader.next_frame(&mut pixels).unwrap_or_else(|e| panic!("decode {}: {e}", path.display()));
    let channels = frame.color_type.samples();
    let rgb = pixels[..frame.buffer_size()].chunks_exact(channels).flat_map(|pixel| [pixel[0], pixel[1], pixel[2]]);
    Picture { width: frame.width, height: frame.height, rgb: rgb.collect() }
}

pub(super) fn png_size(path: &Path) -> (u32, u32) {
    let picture = picture(path);
    (picture.width, picture.height)
}

pub(super) const CELL_WIDTH: u32 = 9;
pub(super) const CELL_HEIGHT: u32 = 20;
pub(super) const MARGIN: u32 = 10;
const CLOSE: u8 = 24;

pub(super) type Rgb = [u8; 3];

pub(super) const BACKGROUND: Rgb = [0x11, 0x11, 0x13];
pub(super) const FOREGROUND: Rgb = [0xd4, 0xd4, 0xd8];
pub(super) const RED: Rgb = [0xf8, 0x71, 0x71];
pub(super) const GREEN: Rgb = [0x4a, 0xde, 0x80];
pub(super) const BLUE: Rgb = [0x60, 0xa5, 0xfa];
pub(super) const XTERM_208: Rgb = [255, 135, 0];
pub(super) const MAGENTA_RGB: Rgb = [255, 0, 255];
pub(super) const REF_COLORS: [Rgb; 6] = [
    [0xfb, 0x92, 0x3c],
    [0x38, 0xbd, 0xf8],
    [0xa3, 0xe6, 0x35],
    [0xf4, 0x72, 0xb6],
    [0xfa, 0xcc, 0x15],
    [0x81, 0x8c, 0xf8],
];

fn close_to(a: Rgb, b: Rgb) -> bool {
    a.iter().zip(b).all(|(a, b)| a.abs_diff(b) <= CLOSE)
}

impl Picture {
    pub(super) fn at(&self, x: u32, y: u32) -> Rgb {
        assert!(x < self.width && y < self.height, "{x},{y} is outside the {}x{} picture", self.width, self.height);
        let at = ((y * self.width + x) * 3) as usize;
        [self.rgb[at], self.rgb[at + 1], self.rgb[at + 2]]
    }

    pub(super) fn cell(&self, row: u32, col: u32) -> Cell<'_> {
        Cell { picture: self, left: MARGIN + (col - 1) * CELL_WIDTH, top: MARGIN + (row - 1) * CELL_HEIGHT }
    }
}

pub(super) struct Cell<'a> {
    picture: &'a Picture,
    left: u32,
    top: u32,
}

impl Cell<'_> {
    pub(super) fn pixels(&self, xs: std::ops::Range<u32>, ys: std::ops::Range<u32>) -> Vec<Rgb> {
        ys.flat_map(|y| xs.clone().map(move |x| (x, y)))
            .map(|(x, y)| self.picture.at(self.left + x, self.top + y))
            .collect()
    }

    fn all(&self) -> Vec<Rgb> {
        self.pixels(0..CELL_WIDTH, 0..CELL_HEIGHT)
    }

    pub(super) fn patch(&self, xs: std::ops::Range<u32>, ys: std::ops::Range<u32>) -> Patch<'_> {
        Patch { cell: self, xs, ys }
    }

    #[track_caller]
    pub(super) fn assert_lines(&self, ends: [bool; 4], what: &str) {
        let (mid_x, mid_y) = (CELL_WIDTH / 2, CELL_HEIGHT / 2);
        let edges = [
            ("top", mid_x - 1..mid_x + 2, 0..1),
            ("right", CELL_WIDTH - 1..CELL_WIDTH, mid_y - 2..mid_y + 3),
            ("bottom", mid_x - 1..mid_x + 2, CELL_HEIGHT - 1..CELL_HEIGHT),
            ("left", 0..1, mid_y - 2..mid_y + 3),
        ];
        for ((edge, xs, ys), reaches) in edges.into_iter().zip(ends) {
            assert_eq!(self.patch(xs, ys).inked(BACKGROUND), reaches, "{what}: a line reaches its {edge} edge");
        }
    }

    pub(super) fn has(&self, color: Rgb) -> bool {
        self.all().into_iter().any(|pixel| close_to(pixel, color))
    }

    pub(super) fn brightness(&self) -> u32 {
        self.all().iter().map(|pixel| pixel.iter().map(|&channel| u32::from(channel)).sum::<u32>()).sum()
    }
}

pub(super) struct Patch<'a> {
    cell: &'a Cell<'a>,
    xs: std::ops::Range<u32>,
    ys: std::ops::Range<u32>,
}

impl Patch<'_> {
    fn pixels(&self) -> Vec<Rgb> {
        self.cell.pixels(self.xs.clone(), self.ys.clone())
    }

    pub(super) fn inked(&self, background: Rgb) -> bool {
        self.pixels().into_iter().any(|pixel| !close_to(pixel, background))
    }

    #[track_caller]
    pub(super) fn assert_filled(&self, color: Rgb, what: &str) {
        let off: Vec<Rgb> = self.pixels().into_iter().filter(|pixel| !close_to(*pixel, color)).collect();
        assert!(off.is_empty(), "{what}: {} pixels are not {color:?}, e.g. {:?}", off.len(), off.first());
    }
}

pub(super) fn pixel(path: &Path, (x, y): (u32, u32)) -> [u8; 3] {
    let picture = picture(path);
    let at = ((y * picture.width + x) * 3) as usize;
    [picture.rgb[at], picture.rgb[at + 1], picture.rgb[at + 2]]
}
