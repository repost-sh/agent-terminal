use super::Canvas;

impl Canvas {
    pub(super) fn block(&mut self, c: char) {
        let (w, h) = (f32::from(self.width), f32::from(self.height));
        let eighth_x = |n: f32| (w * n / 8.0).round();
        let eighth_y = |n: f32| (h * n / 8.0).round();
        let half_y = (h / 2.0).round();
        match c {
            '▀' => self.fill([0.0, 0.0, w, half_y], 255),
            '▁'..='█' => {
                let eighths = eighths_after(c, '▀');
                self.fill([0.0, h - eighth_y(eighths), w, h], 255);
            }
            '▉'..='▏' => {
                let eighths = eighths_before(c, '\u{2590}');
                self.fill([0.0, 0.0, eighth_x(eighths), h], 255);
            }
            '▐' => self.fill([(w / 2.0).round(), 0.0, w, h], 255),
            '░' => self.fill([0.0, 0.0, w, h], 64),
            '▒' => self.fill([0.0, 0.0, w, h], 128),
            '▓' => self.fill([0.0, 0.0, w, h], 192),
            '▔' => self.fill([0.0, 0.0, w, eighth_y(1.0)], 255),
            '▕' => self.fill([w - eighth_x(1.0), 0.0, w, h], 255),
            quadrant => self.quadrants(quadrant_mask(quadrant)),
        }
    }

    fn quadrants(&mut self, [upper_left, upper_right, lower_left, lower_right]: [bool; 4]) {
        let (w, h) = (f32::from(self.width), f32::from(self.height));
        let (half_x, half_y) = ((w / 2.0).round(), (h / 2.0).round());
        for (on, corners) in [
            (upper_left, [0.0, 0.0, half_x, half_y]),
            (upper_right, [half_x, 0.0, w, half_y]),
            (lower_left, [0.0, half_y, half_x, h]),
            (lower_right, [half_x, half_y, w, h]),
        ] {
            if on {
                self.fill(corners, 255);
            }
        }
    }
}

fn eighths_after(c: char, base: char) -> f32 {
    u8::try_from(u32::from(c).saturating_sub(u32::from(base))).map_or(0.0, f32::from)
}

fn eighths_before(c: char, end: char) -> f32 {
    u8::try_from(u32::from(end).saturating_sub(u32::from(c))).map_or(0.0, f32::from)
}

fn quadrant_mask(c: char) -> [bool; 4] {
    match c {
        '▖' => [false, false, true, false],
        '▗' => [false, false, false, true],
        '▘' => [true, false, false, false],
        '▙' => [true, false, true, true],
        '▚' => [true, false, false, true],
        '▛' => [true, true, true, false],
        '▜' => [true, true, false, true],
        '▝' => [false, true, false, false],
        '▞' => [false, true, true, false],
        _ => [false, true, true, true],
    }
}
