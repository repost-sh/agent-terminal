use alacritty_terminal::vte::{Params, Perform};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ShellReport {
    Prompt,
    Ready,
    Started { command: String, cwd: Option<String> },
    Finished { exit_code: Option<i32> },
}

#[derive(Clone, Copy)]
pub(crate) struct At {
    pub text: usize,
    pub styled: usize,
}

pub(crate) struct Printed {
    pub text: String,
    pub styled: String,
}

#[derive(Default)]
pub(super) struct Stream {
    pub(super) cwd: Option<String>,
    pub(super) printed: String,
    pub(super) styled: String,
    pub(super) reports: Vec<(At, ShellReport)>,
}

impl Stream {
    fn report(&mut self, params: &[&[u8]]) {
        let report = match params {
            [b"A", ..] => ShellReport::Prompt,
            [b"B", ..] => ShellReport::Ready,
            [b"C", options @ ..] => ShellReport::Started {
                command: options.iter().find_map(|option| command_line(option)).unwrap_or_default(),
                cwd: self.cwd.clone(),
            },
            [b"D", code @ ..] => ShellReport::Finished {
                exit_code: code.first().and_then(|code| std::str::from_utf8(code).ok()?.parse().ok()),
            },
            _ => return,
        };
        self.reports.push((At { text: self.printed.len(), styled: self.styled.len() }, report));
    }
}

fn command_line(option: &[u8]) -> Option<String> {
    if let Some(encoded) = option.strip_prefix(b"cmdline_url=") {
        return Some(String::from_utf8_lossy(&percent_decoded(encoded)).into_owned());
    }
    option.strip_prefix(b"cmdline=").map(|text| String::from_utf8_lossy(text).into_owned())
}

impl Perform for Stream {
    fn print(&mut self, c: char) {
        self.printed.push(c);
        self.styled.push(c);
    }

    fn execute(&mut self, byte: u8) {
        if matches!(byte, b'\n' | b'\t') {
            self.printed.push(char::from(byte));
            self.styled.push(char::from(byte));
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        if action != 'm' || ignore || !intermediates.is_empty() {
            return;
        }
        let params: Vec<String> =
            params.iter().map(|group| group.iter().map(u16::to_string).collect::<Vec<_>>().join(":")).collect();
        self.styled.push_str("\x1b[");
        self.styled.push_str(&params.join(";"));
        self.styled.push('m');
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        match params {
            [b"7", url @ ..] => {
                if let Some(cwd) = file_url_path(&url.join(&b';')) {
                    self.cwd = Some(cwd);
                }
            }
            [b"133", mark @ ..] => self.report(mark),
            _ => {}
        }
    }
}

fn file_url_path(url: &[u8]) -> Option<String> {
    let rest = url.strip_prefix(b"file://")?;
    let path = rest.get(rest.iter().position(|&b| b == b'/')?..)?;
    Some(String::from_utf8_lossy(&percent_decoded(path)).into_owned())
}

fn percent_decoded(path: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(path.len());
    let mut rest = path;
    while let Some((&first, after)) = rest.split_first() {
        let escaped = if first == b'%' { escape(after) } else { None };
        if let Some((byte, remaining)) = escaped {
            decoded.push(byte);
            rest = remaining;
        } else {
            decoded.push(first);
            rest = after;
        }
    }
    decoded
}

fn escape(after: &[u8]) -> Option<(u8, &[u8])> {
    let (hex, remaining) = after.split_at_checked(2)?;
    Some((u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?, remaining))
}
