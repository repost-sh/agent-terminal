use std::io::{self, ErrorKind, IsTerminal, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

use anyhow::{Context, Result, anyhow, bail};
use rustix::termios::{self, OptionalActions};

use crate::connection::{self, Launch};
use crate::key::{Key, KeyCode, Mods};
use crate::output;
use crate::protocol::request;
use crate::protocol::{
    self, AttachEvent, AttachInput, Attached, Call, HumanInput, Mirror, Reply, Request, Response, Target,
};

const DETACH_FIRST: u8 = 0x10;
const DETACH: [u8; 2] = [DETACH_FIRST, 0x11];
const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";
const TAKE_OVER: &str = "\x1b[?1049h\x1b[?7l\x1b[?2004h\x1b[0m\x1b[2J";
const HAND_BACK: &str = "\x1b[0m\x1b[?2004l\x1b[?7h\x1b[?25h\x1b[?1049l";

enum End {
    Detached,
    Closed,
}

pub(crate) fn run(target: Target, read_only: bool, json: bool, launch: &Launch) -> io::Result<i32> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return output::refuse(
            "attach shows the tab in your terminal, but stdin or stdout is not a terminal; read the screen with `agent-terminal snapshot` instead",
            json,
        );
    }
    let typed = std::env::args_os().skip(1).map(|arg| arg.to_string_lossy().into_owned()).collect();
    let call = Call { typed: Some(typed), request: Request::Attach(request::Attach { target, read_only }) };
    let (response, stream) = match connection::keep_open(&call, launch) {
        Ok(Ok(opened)) => opened,
        Ok(Err(refusal)) => return output::error(&refusal, json),
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
        Ok(end) => report(&attached, &end, json).map(|()| 0),
        Err(e) => output::refuse(&format!("{e:#}"), json),
    }
}

fn report(attached: &Attached, end: &End, json: bool) -> io::Result<()> {
    let (word, text) = match end {
        End::Detached => ("detached", format!("detached from {} {}", attached.session, attached.tab)),
        End::Closed => ("closed", format!("{} {} was closed; detached", attached.session, attached.tab)),
    };
    if json {
        output::print_json(&serde_json::json!({ "session": attached.session, "tab": attached.tab, "end": word }))
    } else {
        output::print(&format!("{text}\n"))
    }
}

fn watch(attached: &Attached, stream: UnixStream) -> Result<End> {
    let keys = stream.try_clone().context("share the connection to the daemon")?;
    let (ended, forwarder, outcome) = taken_over(|| {
        let (report, outcome) = mpsc::channel();
        let forwarder = forward_in_background(keys, attached.read_only, report);
        Ok((mirror(attached, stream), forwarder, outcome))
    })?;
    if let Ok(Err(e)) = outcome.try_recv() {
        return Err(e);
    }
    if forwarder.is_finished() {
        return match forwarder.join() {
            Ok(closed) => closed.and(ended),
            Err(_) => Err(anyhow!("forwarding your keys to the daemon panicked")),
        };
    }
    ended
}

fn forward_in_background(mut keys: UnixStream, read_only: bool, report: Sender<Result<()>>) -> JoinHandle<Result<()>> {
    std::thread::spawn(move || {
        let sent = forward_keys(&mut keys, read_only);
        let failed = sent.is_err();
        if let Err(unheard) = report.send(sent) {
            return unheard.0;
        }
        if failed {
            keys.shutdown(Shutdown::Both).context("close the connection to the daemon")?;
        }
        Ok(())
    })
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

fn forward_keys(daemon: &mut UnixStream, read_only: bool) -> Result<()> {
    let mut stdin = io::stdin().lock();
    let mut keys = Keys::default();
    let mut buffer = [0u8; 4096];
    loop {
        let read = match stdin.read(&mut buffer) {
            Ok(0) => return send(daemon, &AttachInput::Detach),
            Ok(read) => read,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => {
                send(daemon, &AttachInput::Detach)?;
                return Err(anyhow::Error::new(e).context("read your keys"));
            }
        };
        let filled = buffer.get(..read).context("stdin reported more bytes than it was given room for")?;
        for pressed in keys.feed(filled) {
            match pressed {
                Pressed::Detach => return send(daemon, &AttachInput::Detach),
                Pressed::Human(_) if read_only => {}
                Pressed::Human(input) => send(daemon, &AttachInput::Human(input))?,
            }
        }
    }
}

fn send(daemon: &mut UnixStream, input: &AttachInput) -> Result<()> {
    protocol::write(daemon, input).context("send your keys to the daemon")
}

fn taken_over<T>(work: impl FnOnce() -> Result<T>) -> Result<T> {
    let stdin = io::stdin();
    let original = termios::tcgetattr(&stdin).context("read your terminal's settings")?;
    let mut raw = original.clone();
    raw.make_raw();
    termios::tcsetattr(&stdin, OptionalActions::Now, &raw).context("put your terminal in raw mode")?;
    let worked = show(TAKE_OVER).and_then(|()| work());
    let handed_back = show(HAND_BACK).context("restore your terminal's screen");
    let restored =
        termios::tcsetattr(&stdin, OptionalActions::Now, &original).context("restore your terminal's settings");
    match (worked, handed_back.and(restored)) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(e), Ok(())) | (Ok(_), Err(e)) => Err(e),
        (Err(e), Err(restoring)) => Err(e.context(format!("then {restoring:#}"))),
    }
}

fn show(text: &str) -> Result<()> {
    let mut stdout = io::stdout().lock();
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
        let window = termios::tcgetwinsize(io::stdout()).context("read your terminal's size")?;
        let fit = (window.ws_col, window.ws_row, screen.cols, screen.rows);
        let mut paint = String::new();
        if self.fitted != Some(fit) {
            paint.push_str("\x1b[0m\x1b[2J");
            self.shown.clear();
            self.hint.clear();
            self.fitted = Some(fit);
        }
        let visible = usize::from(screen.rows.min(window.ws_row));
        for (index, line) in screen.lines.iter().take(visible).enumerate() {
            if self.shown.get(index) != Some(line) {
                paint.push_str(&crate::paint::row(index, line));
            }
        }
        self.shown = screen.lines.iter().take(visible).cloned().collect();
        if window.ws_row > screen.rows {
            let hint: String = self.hint_for(screen).chars().take(usize::from(window.ws_col)).collect();
            if hint != self.hint {
                paint.push_str(&crate::paint::row(usize::from(screen.rows), &format!("\x1b[2m{hint}\x1b[0m")));
                self.hint = hint;
            }
        }
        let [row, col] = screen.cursor;
        let on_screen = row <= window.ws_row && col <= window.ws_col;
        paint.push_str(&crate::paint::cursor(row, col, screen.cursor_visible && !screen.exited && on_screen));
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
        let mut typing = Typing::default();
        let mut rest = data.as_slice();
        while !rest.is_empty() {
            if self.pasting.is_some() {
                rest = self.paste(rest, &mut typing);
                continue;
            }
            let Some((step, used)) = step(rest) else {
                self.pending = rest.to_vec();
                break;
            };
            rest = rest.get(used..).unwrap_or_default();
            if self.take(&step, &mut typing) == Taken::Detach {
                break;
            }
        }
        typing.finish()
    }

    fn paste<'a>(&mut self, rest: &'a [u8], typing: &mut Typing) -> &'a [u8] {
        let Some(pasted) = &mut self.pasting else {
            return rest;
        };
        let Some((body, after)) = split_around(rest, PASTE_END) else {
            pasted.extend_from_slice(rest);
            return &[];
        };
        pasted.extend_from_slice(body);
        let text = String::from_utf8_lossy(pasted).into_owned();
        self.pasting = None;
        typing.pressed.push(Pressed::Human(HumanInput::Pasted(text)));
        after
    }

    fn take(&mut self, step: &Step, typing: &mut Typing) -> Taken {
        match step {
            Step::Text(c) => typing.text.push(*c),
            Step::Key(key) => typing.press(Pressed::Human(HumanInput::Pressed(*key))),
            Step::Detach => {
                typing.press(Pressed::Detach);
                return Taken::Detach;
            }
            Step::PasteStart => {
                typing.flush();
                self.pasting = Some(Vec::new());
            }
            Step::Skip => {}
        }
        Taken::Key
    }
}

#[derive(PartialEq, Eq)]
enum Taken {
    Key,
    Detach,
}

#[derive(Default)]
struct Typing {
    pressed: Vec<Pressed>,
    text: String,
}

impl Typing {
    fn flush(&mut self) {
        if !self.text.is_empty() {
            self.pressed.push(Pressed::Human(HumanInput::Typed(std::mem::take(&mut self.text))));
        }
    }

    fn press(&mut self, pressed: Pressed) {
        self.flush();
        self.pressed.push(pressed);
    }

    fn finish(mut self) -> Vec<Pressed> {
        self.flush();
        self.pressed
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
    match bytes {
        [] | [DETACH_FIRST] => None,
        [first, second, ..] if [*first, *second] == DETACH => Some((Step::Detach, 2)),
        [0x1b, after @ ..] => escape(bytes, after),
        [byte, ..] if *byte < 0x20 || *byte == 0x7f => Some((Step::Key(control_key(*byte)), 1)),
        [lead, ..] => text(bytes, *lead),
    }
}

fn escape(bytes: &[u8], after: &[u8]) -> Option<(Step, usize)> {
    if bytes.starts_with(PASTE_START) {
        return Some((Step::PasteStart, PASTE_START.len()));
    }
    Some(match after {
        [b'[', ..] => csi(bytes)?,
        [b'O'] => return None,
        [b'O', final_byte, ..] => (ss3_key(*final_byte).map_or(Step::Skip, |code| Step::Key(plain(code))), 3),
        [] | [0x1b, ..] => (Step::Key(plain(KeyCode::Esc)), 1),
        rest => alt(rest)?,
    })
}

fn alt(rest: &[u8]) -> Option<(Step, usize)> {
    Some(match step(rest)? {
        (Step::Key(key), used) => (Step::Key(Key { mods: Mods { alt: true, ..key.mods }, ..key }), used + 1),
        (Step::Text(c), used) => {
            (Step::Key(Key { code: KeyCode::Char(c), mods: Mods { alt: true, ..Mods::default() } }), used + 1)
        }
        (Step::Detach | Step::PasteStart | Step::Skip, _) => (Step::Key(plain(KeyCode::Esc)), 1),
    })
}

fn text(bytes: &[u8], lead: u8) -> Option<(Step, usize)> {
    let width = match lead {
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        0x80..=0xff => return Some((Step::Skip, 1)),
        _ => 1,
    };
    Some(match std::str::from_utf8(bytes.get(..width)?).ok().and_then(|text| text.chars().next()) {
        Some(c) => (Step::Text(c), width),
        None => (Step::Skip, 1),
    })
}

fn csi(bytes: &[u8]) -> Option<(Step, usize)> {
    let body = bytes.get(2..)?;
    let at = body.iter().position(|byte| (0x40..=0x7e).contains(byte))?;
    let (params, after) = body.split_at_checked(at)?;
    let final_byte = *after.first()?;
    let end = at + 2;
    let params = String::from_utf8_lossy(params).into_owned();
    let mut fields = params.split(';').map(|field| field.parse::<u8>().ok());
    let number = fields.next().flatten();
    let held = fields.next().flatten().unwrap_or(1).saturating_sub(1);
    let mods = Mods { shift: held & 1 != 0, alt: held & 2 != 0, ctrl: held & 4 != 0 };
    let code = match (final_byte, number) {
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

fn split_around<'a>(haystack: &'a [u8], needle: &[u8]) -> Option<(&'a [u8], &'a [u8])> {
    let at = haystack.windows(needle.len()).position(|window| window == needle)?;
    let (before, found) = haystack.split_at_checked(at)?;
    Some((before, found.strip_prefix(needle)?))
}
