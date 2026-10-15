use alacritty_terminal::term::TermMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MouseMode {
    X10,
    Sgr,
    Utf8,
}

impl MouseMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            MouseMode::X10 => "x10",
            MouseMode::Sgr => "sgr",
            MouseMode::Utf8 => "utf8",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MouseTracking {
    Click,
    Drag,
    Motion,
}

impl MouseTracking {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            MouseTracking::Click => "click",
            MouseTracking::Drag => "drag",
            MouseTracking::Motion => "motion",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MouseReporting {
    pub tracking: MouseTracking,
    pub encoding: MouseMode,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct KeyboardModes {
    pub app_cursor: bool,
    pub bracketed_paste: bool,
    pub kitty_flags: u8,
}

pub(super) const KITTY_FLAGS: [(TermMode, u8, &str); 5] = [
    (TermMode::DISAMBIGUATE_ESC_CODES, 1, "disambiguate"),
    (TermMode::REPORT_EVENT_TYPES, 2, "report-events"),
    (TermMode::REPORT_ALTERNATE_KEYS, 4, "report-alternates"),
    (TermMode::REPORT_ALL_KEYS_AS_ESC, 8, "report-all-keys"),
    (TermMode::REPORT_ASSOCIATED_TEXT, 16, "report-text"),
];

pub(super) fn mouse_encoding(mode: TermMode) -> Option<MouseMode> {
    mode.intersects(TermMode::MOUSE_MODE).then(|| {
        if mode.contains(TermMode::SGR_MOUSE) {
            MouseMode::Sgr
        } else if mode.contains(TermMode::UTF8_MOUSE) {
            MouseMode::Utf8
        } else {
            MouseMode::X10
        }
    })
}

pub(super) fn mouse_tracking(mode: TermMode) -> Option<MouseTracking> {
    if mode.contains(TermMode::MOUSE_MOTION) {
        Some(MouseTracking::Motion)
    } else if mode.contains(TermMode::MOUSE_DRAG) {
        Some(MouseTracking::Drag)
    } else if mode.contains(TermMode::MOUSE_REPORT_CLICK) {
        Some(MouseTracking::Click)
    } else {
        None
    }
}
