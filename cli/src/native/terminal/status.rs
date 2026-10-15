#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Running,
    Exited(Option<i32>),
}

impl Status {
    pub(crate) fn exited(self) -> bool {
        matches!(self, Status::Exited(_))
    }

    pub(crate) fn code(self) -> Option<i32> {
        match self {
            Status::Exited(code) => code,
            Status::Running => None,
        }
    }
}
