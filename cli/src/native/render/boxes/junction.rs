use super::Canvas;
use super::weights::{DOUBLE, HEAVY, NONE};

#[derive(Clone, Copy)]
pub(super) enum Axis {
    Across,
    Down,
}

#[derive(Clone, Copy)]
pub(super) enum End {
    Start,
    Finish,
}

pub(super) struct Junction {
    pub(super) width: u16,
    pub(super) height: u16,
    pub(super) thin: u16,
}

impl Junction {
    pub(super) fn thick(&self, weight: u8) -> u16 {
        if weight == HEAVY { self.thin * 2 } else { self.thin }
    }

    pub(super) fn size(&self, axis: Axis) -> u16 {
        match axis {
            Axis::Across => self.width,
            Axis::Down => self.height,
        }
    }

    pub(super) fn center(&self, axis: Axis) -> u16 {
        self.size(axis) / 2
    }

    pub(super) fn single(&self, axis: Axis, weight: u8) -> u16 {
        Canvas::across(self.size(axis), self.thick(weight))
    }

    pub(super) fn double(&self, axis: Axis) -> u16 {
        Canvas::across(self.size(axis), 3 * self.thin)
    }

    pub(super) fn crossing(&self, axis: Axis, weight: u8, near: bool, end: End) -> u16 {
        let (double, t) = (self.double(axis), self.thin);
        match (end, weight == DOUBLE, near) {
            (End::Start, true, true) => double + 2 * t,
            (End::Start, true, false) => double,
            (End::Start, false, _) => self.single(axis, weight),
            (End::Finish, true, true) => double + t,
            (End::Finish, true, false) => double + 3 * t,
            (End::Finish, false, _) => self.single(axis, weight) + self.thick(weight),
        }
    }

    pub(super) fn pick(&self, axis: Axis, [near, middle, far]: [u8; 3], end: End) -> u16 {
        match (near, middle, far) {
            (NONE, NONE, NONE) => self.center(axis),
            (NONE, NONE, far) => self.crossing(axis, far, false, end),
            (NONE, _, _) => self.center(axis),
            (near, _, _) => self.crossing(axis, near, true, end),
        }
    }

    pub(super) fn meet(&self, axis: Axis, opposite: u8, [one, other]: [u8; 2], end: End) -> u16 {
        if opposite != NONE && opposite != DOUBLE {
            return self.center(axis);
        }
        if one == DOUBLE || other == DOUBLE {
            let both = one != NONE && other != NONE;
            let t = self.thin;
            return self.double(axis)
                + match (end, both) {
                    (End::Start, true) => 2 * t,
                    (End::Start, false) => 0,
                    (End::Finish, true) => t,
                    (End::Finish, false) => 3 * t,
                };
        }
        let present = [one, other].into_iter().filter(|&weight| weight != NONE);
        let met = match end {
            End::Start => present.map(|weight| self.single(axis, weight)).min(),
            End::Finish => present.map(|weight| self.single(axis, weight) + self.thick(weight)).max(),
        };
        met.unwrap_or_else(|| self.center(axis))
    }
}
