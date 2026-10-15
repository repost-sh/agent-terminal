use std::io::{ErrorKind, IsTerminal, Read, Write};
use std::os::unix::net::UnixStream;

use anyhow::{Context, Result, bail};
use rustix::termios::{self, OptionalActions, Termios};

use crate::connection::{self, Launch};
use crate::key::{Key, KeyCode, Mods};
use crate::output;
use crate::protocol::{
    self, AttachEvent, AttachInput, Attached, Call, HumanInput, Mirror, Reply, Request, Response, Target,
};

const DETACH: [u8; 2] = [0x10, 0x11];
const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";
const TAKE_OVER: &str = "\x1b[?1049h\x1b[?7l\x1b[?2004h\x1b[0m\x1b[2J";
const HAND_BACK: &str = "\x1b[0m\x1b[?2004l\x1b[?7h\x1b[?25h\x1b[?1049l";

enum End {
    Detached,
    Closed,
}

pub fn run(target: Target, read_only: bool, json: bool, launch: Launch) -> i32 {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return output::refuse(
            "attach shows the tab in your terminal, but stdin or stdout is not a terminal; read the screen with `agent-terminal snapshot` instead",
            json,
        );
    }
    let typed = std::env::args_os().skip(1).map(|arg| arg.to_string_lossy().into_owned()).collect();
    let call = Call { typed: Some(typed), request: Request::Attach { target, read_only } };
    let (response, stream) = match connection::keep_open(&call, &launch) {
        Ok(opened) => opened,
        Err(e) => return output::refuse(&format!("{e:#}"), json),
    };
    let attached = match response {
        Response::Ok(Reply::Attached(attached)) => attached,
        Response::Ok(other) => {
            return output::refuse(&format!("the daemon answered attach with {other:?}"), json);
        }
        Response::Err(error) => return output::error(&error, json),
    };
    match watch(&attached, stream) {
        Ok(end) => {
            report(&attached, &end, json);
            0
        }
        Err(e) => output::refuse(&format!("{e:#}"), json),
    }
}

fn report(attached: &Attached, end: &End, json: bool) {
    let (word, text) = match end {
        End::Detached => ("detached", format!("detached from {} {}", attached.session, attached.tab)),
        End::Closed => ("closed", format!("{} {} was closed; detached", attached.session, attached.tab)),
    };
    if json {
        println!("{}", serde_json::json!({ "session": attached.session, "tab": attached.tab, "end": word }));
    } else {
        println!("{text}");
    }
}

fn watch(attached: &Attached, stream: UnixStream) -> Result<End> {
    let keys = stream.try_clone().context("share the connection to the daemon")?;
    let takeover = Takeover::start()?;
    let read_only = attached.read_only;
    std::thread::spawn(move || forward_keys(keys, read_only));
    let ended = mirror(attached, stream);
    drop(takeover);
    ended
}

fn mirror(attached: &Attached, mut stream: UnixStream) -> Result<End> {
    let mut painter = Painter::new(attached);
    loop {
        match protocol::read::<AttachEvent>(&mut stream)? {
            Some(AttachEvent::Screen(screen)) => painter.paint(&screen)?,
            Some(AttachEvent::Detached) => return Ok(End::Detached),
            Some(AttachEvent::Closed) => return Ok(End::Closed),
            None => bail!("the daemon stopped, so {} {} is gone", attached.session, attached.tab),
        }
    }
}

fn forward_keys(mut daemon: UnixStream, read_only: bool) {
    let mut stdin = std::io::stdin().lock();
    let mut keys = Keys::default();
    let mut buffer = [0u8; 4096];
    loop {
        let read = match stdin.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => {
                eprintln!("error: reading your keys failed: {e}");
                break;
            }
        };
        for pressed in keys.feed(&buffer[..read]) {
            let sent = match pressed {
                Pressed::Detach => {
                    send(&mut daemon, &AttachInput::Detach);
                    return;
                }
                Pressed::Human(_) if read_only => true,
                Pressed::Human(input) => send(&mut daemon, &AttachInput::Human(input)),
            };
            if !sent {
                return;
            }
        }
    }
    send(&mut daemon, &AttachInput::Detach);
}

fn send(daemon: &mut UnixStream, input: &AttachInput) -> bool {
    protocol::write(daemon, input).is_ok()
}

struct Takeover {
    original: Termios,
}

impl Takeover {
    fn start() -> Result<Self> {
        let stdin = std::io::stdin();
        let original = termios::tcgetattr(&stdin).context("read your terminal's settings")?;
        let mut raw = original.clone();
        raw.make_raw();
        termios::tcsetattr(&stdin, OptionalActions::Now, &raw).context("put your terminal in raw mode")?;
        let takeover = Self { original };
        show(TAKE_OVER)?;
        Ok(takeover)
    }
}

impl Drop for Takeover {
    fn drop(&mut self) {
        if let Err(e) = show(HAND_BACK) {
            eprintln!("error: restoring your terminal's screen failed: {e:#}");
        }
        if let Err(e) = termios::tcsetattr(std::io::stdin(), OptionalActions::Now, &self.original) {
            eprintln!("error: restoring your terminal's settings failed: {e}");
        }
    }
}

fn show(text: &str) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(text.as_bytes()).and_then(|()| stdout.flush()).context("write to your terminal")
}

struct Painter {
    place: String,
    read_only: bool,
    shown: Vec<String>,
    hint: String,
    fitted: Option<(u16, u16, u16, u16)>,
}

impl Painter {
    fn new(attached: &Attached) -> Self {
        Self {
            place: format!("{} {}", attached.session, attached.tab),
            read_only: attached.read_only,
            shown: Vec::new(),
            hint: String::new(),
            fitted: None,
        }
    }

    fn paint(&mut self, screen: &Mirror) -> Result<()> {
        let window = termios::tcgetwinsize(std::io::stdout()).context("read your terminal's size")?;
        let fit = (window.ws_col, window.ws_row, screen.cols, screen.rows);
        let mut paint = String::new();
        if self.fitted != Some(fit) {
            paint.push_str("\x1b[0m\x1b[2J");
            self.shown.clear();
            self.hint.clear();
            self.fitted = Some(fit);
        }
        let visible = usize::from(screen.rows.min(window.ws_row)).min(screen.lines.len());
        for (index, line) in screen.lines[..visible].iter().enumerate() {
            if self.shown.get(index) != Some(line) {
                paint.push_str(&format!("\x1b[{};1H\x1b[0m\x1b[2K{line}", index + 1));
            }
        }
        self.shown = screen.lines[..visible].to_vec();
        if window.ws_row > screen.rows {
            let hint: String = self.hint_for(screen).chars().take(usize::from(window.ws_col)).collect();
            if hint != self.hint {
                paint.push_str(&format!("\x1b[{};1H\x1b[0m\x1b[2K\x1b[2m{hint}\x1b[0m", screen.rows + 1));
                self.hint = hint;
            }
        }
        let [row, col] = screen.cursor;
        let on_screen = row <= window.ws_row && col <= window.ws_col;
        let visibility = if screen.cursor_visible && !screen.exited && on_screen { 'h' } else { 'l' };
        paint.push_str(&format!("\x1b[0m\x1b[{row};{col}H\x1b[?25{visibility}"));
        show(&paint)
    }

    fn hint_for(&self, screen: &Mirror) -> String {
        const DETACH_HINT: &str = "ctrl-p ctrl-q detaches";
        match (screen.exited, screen.exit_code) {
            (true, Some(code)) => format!("{} exited with code {code} · {DETACH_HINT}", screen.program),
            (true, None) => format!("{} exited · {DETACH_HINT}", screen.program),
            (false, _) if self.read_only => format!("{} · read-only · {DETACH_HINT}", self.place),
            (false, _) => format!("{} · keys go to {} · {DETACH_HINT}", self.place, screen.program),
        }
    }
}

enum Pressed {
    Human(HumanInput),
    Detach,
}

#[derive(Default)]
struct Keys {
    pending: Vec<u8>,
    pasting: Option<Vec<u8>>,
}

impl Keys {
    fn feed(&mut self, bytes: &[u8]) -> Vec<Pressed> {
        let mut data = std::mem::take(&mut self.pending);
        data.extend_from_slice(bytes);
        let mut pressed = Vec::new();
        let mut text = String::new();
        let mut at = 0;
        while at < data.len() {
            let rest = &data[at..];
            if let Some(pasted) = &mut self.pasting {
                let end = find(rest, PASTE_END);
                pasted.extend_from_slice(&rest[..end.unwrap_or(rest.len())]);
                at += end.map_or(rest.len(), |end| end + PASTE_END.len());
                if end.is_some() {
                    let pasted = String::from_utf8_lossy(pasted).into_owned();
                    self.pasting = None;
                    pressed.push(Pressed::Human(HumanInput::Pasted(pasted)));
                }
                continue;
            }
            let Some((step, used)) = step(rest) else {
                self.pending = rest.to_vec();
                break;
            };
            at += used;
            match step {
                Step::Text(c) => text.push(c),
                Step::Key(key) => {
                    typed(&mut pressed, &mut text);
                    pressed.push(Pressed::Human(HumanInput::Pressed(key)));
                }
                Step::Detach => {
                    typed(&mut pressed, &mut text);
                    pressed.push(Pressed::Detach);
                    return pressed;
                }
                Step::PasteStart => {
                    typed(&mut pressed, &mut text);
                    self.pasting = Some(Vec::new());
                }
                Step::Skip => {}
            }
        }
        typed(&mut pressed, &mut text);
        pressed
    }
}

fn typed(pressed: &mut Vec<Pressed>, text: &mut String) {
    if !text.is_empty() {
        pressed.push(Pressed::Human(HumanInput::Typed(std::mem::take(text))));
    }
}

enum Step {
    Text(char),
    Key(Key),
    Detach,
    PasteStart,
    Skip,
}

fn step(bytes: &[u8]) -> Option<(Step, usize)> {
    Some(match bytes {
        [] => return None,
        [first] if *first == DETACH[0] => return None,
        [first, second, ..] if [*first, *second] == DETACH => (Step::Detach, 2),
        [0x1b, ..] if bytes.starts_with(PASTE_START) => (Step::PasteStart, PASTE_START.len()),
        [0x1b, b'[', ..] => csi(bytes)?,
        [0x1b, b'O'] => return None,
        [0x1b, b'O', final_byte, ..] => match ss3_key(*final_byte) {
            Some(code) => (Step::Key(plain(code)), 3),
            None => (Step::Skip, 3),
        },
        [0x1b] | [0x1b, 0x1b, ..] => (Step::Key(plain(KeyCode::Esc)), 1),
        [0x1b, rest @ ..] => match step(rest)? {
            (Step::Key(key), used) => (Step::Key(Key { mods: Mods { alt: true, ..key.mods }, ..key }), used + 1),
            (Step::Text(c), used) => {
                (Step::Key(Key { code: KeyCode::Char(c), mods: Mods { alt: true, ..Mods::default() } }), used + 1)
            }
            (Step::Detach | Step::PasteStart | Step::Skip, _) => (Step::Key(plain(KeyCode::Esc)), 1),
        },
        [byte, ..] if *byte < 0x20 || *byte == 0x7f => (Step::Key(control_key(*byte)), 1),
        [byte, ..] => {
            let width = match byte {
                0xc0..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf7 => 4,
                0x80..=0xff => return Some((Step::Skip, 1)),
                _ => 1,
            };
            match std::str::from_utf8(bytes.get(..width)?).ok().and_then(|text| text.chars().next()) {
                Some(c) => (Step::Text(c), width),
                None => (Step::Skip, 1),
            }
        }
    })
}

fn csi(bytes: &[u8]) -> Option<(Step, usize)> {
    let end = bytes.iter().skip(2).position(|byte| (0x40..=0x7e).contains(byte))? + 2;
    let params = String::from_utf8_lossy(&bytes[2..end]).into_owned();
    let mut fields = params.split(';').map(|field| field.parse::<u8>().ok());
    let number = fields.next().flatten();
    let held = fields.next().flatten().unwrap_or(1).saturating_sub(1);
    let mods = Mods { shift: held & 1 != 0, alt: held & 2 != 0, ctrl: held & 4 != 0 };
    let code = match (bytes[end], number) {
        (b'Z', _) => return Some((Step::Key(Key { code: KeyCode::Tab, mods: Mods { shift: true, ..mods } }), end + 1)),
        (b'~', Some(number)) => tilde_key(number),
        (final_byte, _) => ss3_key(final_byte),
    };
    Some(match code {
        Some(code) => (Step::Key(Key { code, mods }), end + 1),
        None => (Step::Skip, end + 1),
    })
}

fn ss3_key(final_byte: u8) -> Option<KeyCode> {
    Some(match final_byte {
        b'A' => KeyCode::Up,
        b'B' => KeyCode::Down,
        b'C' => KeyCode::Right,
        b'D' => KeyCode::Left,
        b'H' => KeyCode::Home,
        b'F' => KeyCode::End,
        b'P' => KeyCode::F(1),
        b'Q' => KeyCode::F(2),
        b'R' => KeyCode::F(3),
        b'S' => KeyCode::F(4),
        _ => return None,
    })
}

fn tilde_key(number: u8) -> Option<KeyCode> {
    Some(match number {
        1 | 7 => KeyCode::Home,
        2 => KeyCode::Insert,
        3 => KeyCode::Delete,
        4 | 8 => KeyCode::End,
        5 => KeyCode::PageUp,
        6 => KeyCode::PageDown,
        11..=15 => KeyCode::F(number - 10),
        17..=21 => KeyCode::F(number - 11),
        23 | 24 => KeyCode::F(number - 12),
        _ => return None,
    })
}

fn control_key(byte: u8) -> Key {
    match byte {
        b'\r' => plain(KeyCode::Enter),
        b'\t' => plain(KeyCode::Tab),
        0x7f => plain(KeyCode::Backspace),
        0 => ctrl(' '),
        0x01..=0x1a => ctrl(char::from(b'a' + byte - 1)),
        0x1c => ctrl('\\'),
        0x1d => ctrl(']'),
        0x1e => ctrl('^'),
        _ => ctrl('_'),
    }
}

fn plain(code: KeyCode) -> Key {
    Key { code, mods: Mods::default() }
}

fn ctrl(c: char) -> Key {
    Key { code: KeyCode::Char(c), mods: Mods { ctrl: true, ..Mods::default() } }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}
