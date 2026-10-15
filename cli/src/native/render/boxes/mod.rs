mod blocks;
mod junction;
mod weights;

use tiny_skia::{LineCap, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use super::fonts::Mask;
use junction::{Axis, End, Junction};
use weights::{DOUBLE, LINES, NONE};

fn dashes(c: char) -> Option<u8> {
    match c {
        '┄' | '┅' | '┆' | '┇' => Some(3),
        '┈' | '┉' | '┊' | '┋' => Some(4),
        '╌' | '╍' | '╎' | '╏' => Some(2),
        _ => None,
    }
}

pub(super) fn mask(c: char, width: u16, height: u16, scale: u8) -> Option<Mask> {
    let code = u32::from(c);
    if !(0x2500..=0x259f).contains(&code) {
        return None;
    }
    let mut canvas = Canvas::new(width, height, scale)?;
    match code {
        0x2580..=0x259f => canvas.block(c),
        0x256d..=0x2570 => canvas.arc(c),
        0x2571..=0x2573 => canvas.diagonals(c),
        _ => {
            let arms = *LINES.get(usize::try_from(code - 0x2500).ok()?)?;
            match dashes(c) {
                Some(count) => canvas.dashed(arms, count),
                None => canvas.lines(arms),
            }
        }
    }
    Some(canvas.into_mask())
}

struct Canvas {
    pixmap: Pixmap,
    width: u16,
    height: u16,
    thin: u16,
}

impl Canvas {
    fn new(width: u16, height: u16, scale: u8) -> Option<Self> {
        let pixmap = Pixmap::new(u32::from(width), u32::from(height))?;
        Some(Self { pixmap, width, height, thin: u16::from(scale.max(1)) })
    }

    fn into_mask(self) -> Mask {
        let alpha = self.pixmap.pixels().iter().map(|pixel| pixel.alpha()).collect();
        Mask { left: 0, top: 0, width: u32::from(self.width), height: u32::from(self.height), alpha }
    }

    fn paint(coverage: u8) -> Paint<'static> {
        let mut paint = Paint::default();
        paint.set_color_rgba8(255, 255, 255, coverage);
        paint.anti_alias = true;
        paint
    }

    fn fill(&mut self, [x0, y0, x1, y1]: [f32; 4], coverage: u8) {
        if let Some(rect) = Rect::from_ltrb(x0, y0, x1, y1) {
            self.pixmap.fill_rect(rect, &Self::paint(coverage), Transform::identity(), None);
        }
    }

    fn junction(&self) -> Junction {
        Junction { width: self.width, height: self.height, thin: self.thin }
    }

    fn thickness(&self, weight: u8) -> u16 {
        self.junction().thick(weight)
    }

    fn across(size: u16, thickness: u16) -> u16 {
        (size - thickness.min(size)) / 2
    }

    fn lines(&mut self, arms: [u8; 4]) {
        let junction = self.junction();
        let [up, right, down, left] = arms;
        if right != NONE {
            self.right(&junction, arms);
        }
        if left != NONE {
            self.left(&junction, arms);
        }
        if up != NONE {
            self.up(&junction, arms);
        }
        if down != NONE {
            self.down(&junction, arms);
        }
    }

    fn right(&mut self, j: &Junction, [up, right, down, left]: [u8; 4]) {
        if right == DOUBLE {
            let top = j.pick(Axis::Across, [up, left, down], End::Start);
            let bottom = j.pick(Axis::Across, [down, left, up], End::Start);
            self.span_x(top, j.width, j.double(Axis::Down), j.thin);
            self.span_x(bottom, j.width, j.double(Axis::Down) + 2 * j.thin, j.thin);
        } else {
            let start = j.meet(Axis::Across, left, [up, down], End::Start);
            self.span_x(start, j.width, j.single(Axis::Down, right), j.thick(right));
        }
    }

    fn left(&mut self, j: &Junction, [up, right, down, left]: [u8; 4]) {
        if left == DOUBLE {
            let top = j.pick(Axis::Across, [up, right, down], End::Finish);
            let bottom = j.pick(Axis::Across, [down, right, up], End::Finish);
            self.span_x(0, top, j.double(Axis::Down), j.thin);
            self.span_x(0, bottom, j.double(Axis::Down) + 2 * j.thin, j.thin);
        } else {
            let end = j.meet(Axis::Across, right, [up, down], End::Finish);
            self.span_x(0, end, j.single(Axis::Down, left), j.thick(left));
        }
    }

    fn up(&mut self, j: &Junction, [up, right, down, left]: [u8; 4]) {
        if up == DOUBLE {
            let left_line = j.pick(Axis::Down, [left, down, right], End::Finish);
            let right_line = j.pick(Axis::Down, [right, down, left], End::Finish);
            self.span_y(0, left_line, j.double(Axis::Across), j.thin);
            self.span_y(0, right_line, j.double(Axis::Across) + 2 * j.thin, j.thin);
        } else {
            let end = j.meet(Axis::Down, down, [left, right], End::Finish);
            self.span_y(0, end, j.single(Axis::Across, up), j.thick(up));
        }
    }

    fn down(&mut self, j: &Junction, [up, right, down, left]: [u8; 4]) {
        if down == DOUBLE {
            let left_line = j.pick(Axis::Down, [left, up, right], End::Start);
            let right_line = j.pick(Axis::Down, [right, up, left], End::Start);
            self.span_y(left_line, j.height, j.double(Axis::Across), j.thin);
            self.span_y(right_line, j.height, j.double(Axis::Across) + 2 * j.thin, j.thin);
        } else {
            let start = j.meet(Axis::Down, up, [left, right], End::Start);
            self.span_y(start, j.height, j.single(Axis::Across, down), j.thick(down));
        }
    }

    fn span_x(&mut self, x0: u16, x1: u16, y: u16, thickness: u16) {
        self.fill([f32::from(x0), f32::from(y), f32::from(x1), f32::from(y + thickness)], 255);
    }

    fn span_y(&mut self, y0: u16, y1: u16, x: u16, thickness: u16) {
        self.fill([f32::from(x), f32::from(y0), f32::from(x + thickness), f32::from(y1)], 255);
    }

    fn dashed(&mut self, [up, right, ..]: [u8; 4], count: u8) {
        let (w, h) = (f32::from(self.width), f32::from(self.height));
        if right == NONE {
            let thickness = self.thickness(up);
            let x = f32::from(Self::across(self.width, thickness));
            for (start, end) in dash_ends(h, count) {
                self.fill([x, start, x + f32::from(thickness), end], 255);
            }
        } else {
            let thickness = self.thickness(right);
            let y = f32::from(Self::across(self.height, thickness));
            for (start, end) in dash_ends(w, count) {
                self.fill([start, y, end, y + f32::from(thickness)], 255);
            }
        }
    }

    fn stroke(&mut self, path: &tiny_skia::Path) {
        let stroke = Stroke { width: f32::from(self.thin), line_cap: LineCap::Butt, ..Stroke::default() };
        self.pixmap.stroke_path(path, &Self::paint(255), &stroke, Transform::identity(), None);
    }

    fn arc(&mut self, corner: char) {
        let (width, height) = (f32::from(self.width), f32::from(self.height));
        let half_line = f32::from(self.thin) / 2.0;
        let x = f32::from(Self::across(self.width, self.thin)) + half_line;
        let y = f32::from(Self::across(self.height, self.thin)) + half_line;
        let radius = (width / 2.0).min(height / 2.0);
        let (to_x, to_y) = match corner {
            '╭' => (width, height),
            '╮' => (0.0, height),
            '╯' => (0.0, 0.0),
            _ => (width, 0.0),
        };
        let toward_x = if to_x > x { 1.0 } else { -1.0 };
        let toward_y = if to_y > y { 1.0 } else { -1.0 };
        let handle = radius * 0.447_715;
        let mut path = PathBuilder::new();
        path.move_to(x, to_y);
        path.line_to(x, y + toward_y * radius);
        path.cubic_to(x, y + toward_y * handle, x + toward_x * handle, y, x + toward_x * radius, y);
        path.line_to(to_x, y);
        if let Some(path) = path.finish() {
            self.stroke(&path);
        }
    }

    fn diagonals(&mut self, c: char) {
        let (w, h) = (f32::from(self.width), f32::from(self.height));
        let mut path = PathBuilder::new();
        if c != '╲' {
            path.move_to(w, 0.0);
            path.line_to(0.0, h);
        }
        if c != '╱' {
            path.move_to(0.0, 0.0);
            path.line_to(w, h);
        }
        if let Some(path) = path.finish() {
            self.stroke(&path);
        }
    }
}

fn dash_ends(length: f32, count: u8) -> impl Iterator<Item = (f32, f32)> {
    let step = length / f32::from(count);
    let gap = (step / 3.0).max(1.0).round();
    (0..count).map(move |dash| {
        let start = (f32::from(dash) * step + gap / 2.0).round();
        let end = ((f32::from(dash) + 1.0) * step - gap / 2.0).round();
        (start, end)
    })
}
