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

#[derive(Clone, Copy)]
pub(super) struct Side {
    pub(super) axis: Axis,
    pub(super) end: End,
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

    fn crossing(&self, Side { axis, end }: Side, weight: u8, near: bool) -> u16 {
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

    pub(super) fn pick(&self, side: Side, [near, middle, far]: [u8; 3]) -> u16 {
        match (near, middle, far) {
            (NONE, NONE, NONE) => self.center(side.axis),
            (NONE, NONE, far) => self.crossing(side, far, false),
            (NONE, _, _) => self.center(side.axis),
            (near, _, _) => self.crossing(side, near, true),
        }
    }

    fn past_double(&self, end: End, both: bool) -> u16 {
        let t = self.thin;
        match (end, both) {
            (End::Start, true) => 2 * t,
            (End::Start, false) => 0,
            (End::Finish, true) => t,
            (End::Finish, false) => 3 * t,
        }
    }

    pub(super) fn meet(&self, Side { axis, end }: Side, [opposite, one, other]: [u8; 3]) -> u16 {
        if opposite != NONE && opposite != DOUBLE {
            return self.center(axis);
        }
        if one == DOUBLE || other == DOUBLE {
            return self.double(axis) + self.past_double(end, one != NONE && other != NONE);
        }
        self.meet_singles(axis, [one, other], end)
    }

    fn meet_singles(&self, axis: Axis, weights: [u8; 2], end: End) -> u16 {
        let present = weights.into_iter().filter(|&weight| weight != NONE);
        let met = match end {
            End::Start => present.map(|weight| self.single(axis, weight)).min(),
            End::Finish => present.map(|weight| self.single(axis, weight) + self.thick(weight)).max(),
        };
        met.unwrap_or_else(|| self.center(axis))
    }
}
