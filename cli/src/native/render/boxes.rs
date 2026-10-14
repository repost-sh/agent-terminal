use tiny_skia::{LineCap, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use super::fonts::Mask;

const NONE: u8 = 0;
const HEAVY: u8 = 2;
const DOUBLE: u8 = 3;

const LINES: [[u8; 4]; 128] = [
    [0, 1, 0, 1],
    [0, 2, 0, 2],
    [1, 0, 1, 0],
    [2, 0, 2, 0],
    [0, 1, 0, 1],
    [0, 2, 0, 2],
    [1, 0, 1, 0],
    [2, 0, 2, 0],
    [0, 1, 0, 1],
    [0, 2, 0, 2],
    [1, 0, 1, 0],
    [2, 0, 2, 0],
    [0, 1, 1, 0],
    [0, 2, 1, 0],
    [0, 1, 2, 0],
    [0, 2, 2, 0],
    [0, 0, 1, 1],
    [0, 0, 1, 2],
    [0, 0, 2, 1],
    [0, 0, 2, 2],
    [1, 1, 0, 0],
    [1, 2, 0, 0],
    [2, 1, 0, 0],
    [2, 2, 0, 0],
    [1, 0, 0, 1],
    [1, 0, 0, 2],
    [2, 0, 0, 1],
    [2, 0, 0, 2],
    [1, 1, 1, 0],
    [1, 2, 1, 0],
    [2, 1, 1, 0],
    [1, 1, 2, 0],
    [2, 1, 2, 0],
    [2, 2, 1, 0],
    [1, 2, 2, 0],
    [2, 2, 2, 0],
    [1, 0, 1, 1],
    [1, 0, 1, 2],
    [2, 0, 1, 1],
    [1, 0, 2, 1],
    [2, 0, 2, 1],
    [2, 0, 1, 2],
    [1, 0, 2, 2],
    [2, 0, 2, 2],
    [0, 1, 1, 1],
    [0, 1, 1, 2],
    [0, 2, 1, 1],
    [0, 2, 1, 2],
    [0, 1, 2, 1],
    [0, 1, 2, 2],
    [0, 2, 2, 1],
    [0, 2, 2, 2],
    [1, 1, 0, 1],
    [1, 1, 0, 2],
    [1, 2, 0, 1],
    [1, 2, 0, 2],
    [2, 1, 0, 1],
    [2, 1, 0, 2],
    [2, 2, 0, 1],
    [2, 2, 0, 2],
    [1, 1, 1, 1],
    [1, 1, 1, 2],
    [1, 2, 1, 1],
    [1, 2, 1, 2],
    [2, 1, 1, 1],
    [1, 1, 2, 1],
    [2, 1, 2, 1],
    [2, 1, 1, 2],
    [2, 2, 1, 1],
    [1, 1, 2, 2],
    [1, 2, 2, 1],
    [2, 2, 1, 2],
    [1, 2, 2, 2],
    [2, 1, 2, 2],
    [2, 2, 2, 1],
    [2, 2, 2, 2],
    [0, 1, 0, 1],
    [0, 2, 0, 2],
    [1, 0, 1, 0],
    [2, 0, 2, 0],
    [0, 3, 0, 3],
    [3, 0, 3, 0],
    [0, 3, 1, 0],
    [0, 1, 3, 0],
    [0, 3, 3, 0],
    [0, 0, 1, 3],
    [0, 0, 3, 1],
    [0, 0, 3, 3],
    [1, 3, 0, 0],
    [3, 1, 0, 0],
    [3, 3, 0, 0],
    [1, 0, 0, 3],
    [3, 0, 0, 1],
    [3, 0, 0, 3],
    [1, 3, 1, 0],
    [3, 1, 3, 0],
    [3, 3, 3, 0],
    [1, 0, 1, 3],
    [3, 0, 3, 1],
    [3, 0, 3, 3],
    [0, 3, 1, 3],
    [0, 1, 3, 1],
    [0, 3, 3, 3],
    [1, 3, 0, 3],
    [3, 1, 0, 1],
    [3, 3, 0, 3],
    [1, 3, 1, 3],
    [3, 1, 3, 1],
    [3, 3, 3, 3],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 1],
    [1, 0, 0, 0],
    [0, 1, 0, 0],
    [0, 0, 1, 0],
    [0, 0, 0, 2],
    [2, 0, 0, 0],
    [0, 2, 0, 0],
    [0, 0, 2, 0],
    [0, 2, 0, 1],
    [1, 0, 2, 0],
    [0, 1, 0, 2],
    [2, 0, 1, 0],
];

fn dashes(c: char) -> Option<u32> {
    match c {
        '┄' | '┅' | '┆' | '┇' => Some(3),
        '┈' | '┉' | '┊' | '┋' => Some(4),
        '╌' | '╍' | '╎' | '╏' => Some(2),
        _ => None,
    }
}

pub fn mask(c: char, width: u32, height: u32, scale: u8) -> Option<Mask> {
    let code = c as u32;
    if !(0x2500..=0x259f).contains(&code) {
        return None;
    }
    let mut canvas = Canvas::new(width, height, scale)?;
    match code {
        0x2580..=0x259f => canvas.block(c),
        0x256d..=0x2570 => canvas.arc(c),
        0x2571..=0x2573 => canvas.diagonals(c),
        _ => {
            let arms = LINES[(code - 0x2500) as usize];
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
    width: u32,
    height: u32,
    thin: u32,
}

impl Canvas {
    fn new(width: u32, height: u32, scale: u8) -> Option<Self> {
        Some(Self { pixmap: Pixmap::new(width, height)?, width, height, thin: u32::from(scale.max(1)) })
    }

    fn into_mask(self) -> Mask {
        let alpha = self.pixmap.pixels().iter().map(|pixel| pixel.alpha()).collect();
        Mask { left: 0, top: 0, width: self.width, height: self.height, alpha }
    }

    fn paint(coverage: u8) -> Paint<'static> {
        let mut paint = Paint::default();
        paint.set_color_rgba8(255, 255, 255, coverage);
        paint.anti_alias = true;
        paint
    }

    fn fill(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, coverage: u8) {
        if let Some(rect) = Rect::from_ltrb(x0, y0, x1, y1) {
            self.pixmap.fill_rect(rect, &Self::paint(coverage), Transform::identity(), None);
        }
    }

    fn thickness(&self, weight: u8) -> u32 {
        if weight == HEAVY { self.thin * 2 } else { self.thin }
    }

    fn across(size: u32, thickness: u32) -> u32 {
        (size - thickness.min(size)) / 2
    }

    fn lines(&mut self, [up, right, down, left]: [u8; 4]) {
        let (w, h, t) = (self.width, self.height, self.thin);
        let thick = |weight: u8| if weight == HEAVY { t * 2 } else { t };
        let x_single = |weight: u8| Self::across(w, thick(weight));
        let y_single = |weight: u8| Self::across(h, thick(weight));
        let x_double = Self::across(w, 3 * t);
        let y_double = Self::across(h, 3 * t);
        let vertical = [up, down].into_iter().filter(|&weight| weight != NONE);
        let horizontal = [left, right].into_iter().filter(|&weight| weight != NONE);
        let any_vertical_double = up == DOUBLE || down == DOUBLE;
        let any_horizontal_double = left == DOUBLE || right == DOUBLE;
        let both_vertical = up != NONE && down != NONE;
        let both_horizontal = left != NONE && right != NONE;

        if right != NONE {
            if right == DOUBLE {
                let top = if up != NONE {
                    if up == DOUBLE { x_double + 2 * t } else { x_single(up) }
                } else if left != NONE {
                    w / 2
                } else if down != NONE {
                    if down == DOUBLE { x_double } else { x_single(down) }
                } else {
                    w / 2
                };
                let bottom = if down != NONE {
                    if down == DOUBLE { x_double + 2 * t } else { x_single(down) }
                } else if left != NONE {
                    w / 2
                } else if up != NONE {
                    if up == DOUBLE { x_double } else { x_single(up) }
                } else {
                    w / 2
                };
                self.span_x(top, w, y_double, t);
                self.span_x(bottom, w, y_double + 2 * t, t);
            } else {
                let start = if left != NONE && left != DOUBLE {
                    w / 2
                } else if any_vertical_double {
                    if both_vertical { x_double + 2 * t } else { x_double }
                } else {
                    vertical.clone().map(x_single).min().unwrap_or(w / 2)
                };
                self.span_x(start, w, y_single(right), thick(right));
            }
        }
        if left != NONE {
            if left == DOUBLE {
                let top = if up != NONE {
                    if up == DOUBLE { x_double + t } else { x_single(up) + thick(up) }
                } else if right != NONE {
                    w / 2
                } else if down != NONE {
                    if down == DOUBLE { x_double + 3 * t } else { x_single(down) + thick(down) }
                } else {
                    w / 2
                };
                let bottom = if down != NONE {
                    if down == DOUBLE { x_double + t } else { x_single(down) + thick(down) }
                } else if right != NONE {
                    w / 2
                } else if up != NONE {
                    if up == DOUBLE { x_double + 3 * t } else { x_single(up) + thick(up) }
                } else {
                    w / 2
                };
                self.span_x(0, top, y_double, t);
                self.span_x(0, bottom, y_double + 2 * t, t);
            } else {
                let end = if right != NONE && right != DOUBLE {
                    w / 2
                } else if any_vertical_double {
                    if both_vertical { x_double + t } else { x_double + 3 * t }
                } else {
                    vertical.clone().map(|weight| x_single(weight) + thick(weight)).max().unwrap_or(w / 2)
                };
                self.span_x(0, end, y_single(left), thick(left));
            }
        }
        if up != NONE {
            if up == DOUBLE {
                let left_line = if left != NONE {
                    if left == DOUBLE { y_double + t } else { y_single(left) + thick(left) }
                } else if down != NONE {
                    h / 2
                } else if right != NONE {
                    if right == DOUBLE { y_double + 3 * t } else { y_single(right) + thick(right) }
                } else {
                    h / 2
                };
                let right_line = if right != NONE {
                    if right == DOUBLE { y_double + t } else { y_single(right) + thick(right) }
                } else if down != NONE {
                    h / 2
                } else if left != NONE {
                    if left == DOUBLE { y_double + 3 * t } else { y_single(left) + thick(left) }
                } else {
                    h / 2
                };
                self.span_y(0, left_line, x_double, t);
                self.span_y(0, right_line, x_double + 2 * t, t);
            } else {
                let end = if down != NONE && down != DOUBLE {
                    h / 2
                } else if any_horizontal_double {
                    if both_horizontal { y_double + t } else { y_double + 3 * t }
                } else {
                    horizontal.clone().map(|weight| y_single(weight) + thick(weight)).max().unwrap_or(h / 2)
                };
                self.span_y(0, end, x_single(up), thick(up));
            }
        }
        if down != NONE {
            if down == DOUBLE {
                let left_line = if left != NONE {
                    if left == DOUBLE { y_double + 2 * t } else { y_single(left) }
                } else if up != NONE {
                    h / 2
                } else if right != NONE {
                    if right == DOUBLE { y_double } else { y_single(right) }
                } else {
                    h / 2
                };
                let right_line = if right != NONE {
                    if right == DOUBLE { y_double + 2 * t } else { y_single(right) }
                } else if up != NONE {
                    h / 2
                } else if left != NONE {
                    if left == DOUBLE { y_double } else { y_single(left) }
                } else {
                    h / 2
                };
                self.span_y(left_line, h, x_double, t);
                self.span_y(right_line, h, x_double + 2 * t, t);
            } else {
                let start = if up != NONE && up != DOUBLE {
                    h / 2
                } else if any_horizontal_double {
                    if both_horizontal { y_double + 2 * t } else { y_double }
                } else {
                    horizontal.map(y_single).min().unwrap_or(h / 2)
                };
                self.span_y(start, h, x_single(down), thick(down));
            }
        }
    }

    fn span_x(&mut self, x0: u32, x1: u32, y: u32, thickness: u32) {
        self.fill(x0 as f32, y as f32, x1 as f32, (y + thickness) as f32, 255);
    }

    fn span_y(&mut self, y0: u32, y1: u32, x: u32, thickness: u32) {
        self.fill(x as f32, y0 as f32, (x + thickness) as f32, y1 as f32, 255);
    }

    fn dashed(&mut self, [up, right, ..]: [u8; 4], count: u32) {
        let (w, h) = (self.width as f32, self.height as f32);
        if right != NONE {
            let thickness = self.thickness(right);
            let y = Self::across(self.height, thickness) as f32;
            let step = w / count as f32;
            let gap = (step / 3.0).max(1.0).round();
            for dash in 0..count {
                let x0 = (dash as f32 * step + gap / 2.0).round();
                let x1 = ((dash + 1) as f32 * step - gap / 2.0).round();
                self.fill(x0, y, x1, y + thickness as f32, 255);
            }
        } else {
            let thickness = self.thickness(up);
            let x = Self::across(self.width, thickness) as f32;
            let step = h / count as f32;
            let gap = (step / 3.0).max(1.0).round();
            for dash in 0..count {
                let y0 = (dash as f32 * step + gap / 2.0).round();
                let y1 = ((dash + 1) as f32 * step - gap / 2.0).round();
                self.fill(x, y0, x + thickness as f32, y1, 255);
            }
        }
    }

    fn stroke(&mut self, path: tiny_skia::Path) {
        let stroke = Stroke { width: self.thin as f32, line_cap: LineCap::Butt, ..Stroke::default() };
        self.pixmap.stroke_path(&path, &Self::paint(255), &stroke, Transform::identity(), None);
    }

    fn arc(&mut self, c: char) {
        let (w, h) = (self.width as f32, self.height as f32);
        let x = Self::across(self.width, self.thin) as f32 + self.thin as f32 / 2.0;
        let y = Self::across(self.height, self.thin) as f32 + self.thin as f32 / 2.0;
        let radius = (w / 2.0).min(h / 2.0);
        let (to_x, to_y) = match c {
            '╭' => (w, h),
            '╮' => (0.0, h),
            '╯' => (0.0, 0.0),
            _ => (w, 0.0),
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
            self.stroke(path);
        }
    }

    fn diagonals(&mut self, c: char) {
        let (w, h) = (self.width as f32, self.height as f32);
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
            self.stroke(path);
        }
    }

    fn block(&mut self, c: char) {
        let (w, h) = (self.width as f32, self.height as f32);
        let eighth_x = |n: f32| (w * n / 8.0).round();
        let eighth_y = |n: f32| (h * n / 8.0).round();
        let half_x = (w / 2.0).round();
        let half_y = (h / 2.0).round();
        let quadrants =
            |canvas: &mut Self, upper_left: bool, upper_right: bool, lower_left: bool, lower_right: bool| {
                for (on, x0, y0, x1, y1) in [
                    (upper_left, 0.0, 0.0, half_x, half_y),
                    (upper_right, half_x, 0.0, w, half_y),
                    (lower_left, 0.0, half_y, half_x, h),
                    (lower_right, half_x, half_y, w, h),
                ] {
                    if on {
                        canvas.fill(x0, y0, x1, y1, 255);
                    }
                }
            };
        match c {
            '▀' => self.fill(0.0, 0.0, w, half_y, 255),
            '▁'..='█' => {
                let eighths = (c as u32 - '▀' as u32) as f32;
                self.fill(0.0, h - eighth_y(eighths), w, h, 255);
            }
            '▉'..='▏' => {
                let eighths = (0x2590 - c as u32) as f32;
                self.fill(0.0, 0.0, eighth_x(eighths), h, 255);
            }
            '▐' => self.fill(half_x, 0.0, w, h, 255),
            '░' => self.fill(0.0, 0.0, w, h, 64),
            '▒' => self.fill(0.0, 0.0, w, h, 128),
            '▓' => self.fill(0.0, 0.0, w, h, 192),
            '▔' => self.fill(0.0, 0.0, w, eighth_y(1.0), 255),
            '▕' => self.fill(w - eighth_x(1.0), 0.0, w, h, 255),
            '▖' => quadrants(self, false, false, true, false),
            '▗' => quadrants(self, false, false, false, true),
            '▘' => quadrants(self, true, false, false, false),
            '▙' => quadrants(self, true, false, true, true),
            '▚' => quadrants(self, true, false, false, true),
            '▛' => quadrants(self, true, true, true, false),
            '▜' => quadrants(self, true, true, false, true),
            '▝' => quadrants(self, false, true, false, false),
            '▞' => quadrants(self, false, true, true, false),
            _ => quadrants(self, false, true, true, true),
        }
    }
}
