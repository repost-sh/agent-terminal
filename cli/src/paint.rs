pub(crate) fn row(index: usize, text: &str) -> String {
    format!("\x1b[{};1H\x1b[0m\x1b[2K{text}", index + 1)
}

pub(crate) fn cursor([row, col]: [u16; 2], visible: bool) -> String {
    let visibility = if visible { 'h' } else { 'l' };
    format!("\x1b[0m\x1b[{row};{col}H\x1b[?25{visibility}")
}
