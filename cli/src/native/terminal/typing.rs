use std::io::Write;
use std::sync::{MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};

use super::state::State;
use super::{Status, Terminal};
use crate::key::{Key, KeyCode, Mods};
use crate::native::keyboard;
use crate::native::mouse::{self, Action};
use crate::native::wait;
use crate::protocol::{ClickTarget, Delivery, Direction, Input, MouseButton, MouseEvent, ScrollBy, Signal};

struct Click<'a> {
    target: &'a ClickTarget,
    button: MouseButton,
    double: bool,
    mods: Mods,
}

impl Terminal {
    pub(crate) fn input(&self, input: &Input) -> Result<Delivery> {
        let _turn = self.input_turn.lock().unwrap_or_else(PoisonError::into_inner);
        match input {
            Input::Type { text, delay_ms } => self.type_text(text, Duration::from_millis(*delay_ms)),
            Input::Press { keys, repeat } => self.press_keys(keys, *repeat),
            Input::Paste { text } => self.paste(text),
            Input::Send { bytes } => {
                self.write(bytes)?;
                Ok(Delivery::Raw { bytes: bytes.len() })
            }
            Input::Signal { signal } => self.signal(*signal),
            Input::Click { target, button, double, mods } => {
                self.click(&Click { target, button: *button, double: *double, mods: *mods })
            }
            Input::Mouse { event } => self.mouse(*event),
            Input::Scroll { direction, count, keys } => self.scroll(*direction, *count, *keys),
            Input::Focus { focused } => self.focus(*focused),
        }
    }

    fn type_text(&self, text: &str, delay: Duration) -> Result<Delivery> {
        let keys = Key::for_text(text).map_err(anyhow::Error::msg)?;
        let mut bytes = 0;
        for (i, key) in keys.iter().enumerate() {
            if i > 0 {
                self.pause(delay)?;
            }
            bytes += self.press(*key)?;
        }
        Ok(Delivery::Text { keys: keys.len(), bytes })
    }

    fn press_keys(&self, keys: &[Key], repeat: u32) -> Result<Delivery> {
        let modes = self.state.lock().unwrap_or_else(PoisonError::into_inner).screen.keyboard_modes();
        let encoded = keys
            .iter()
            .map(|key| keyboard::encode(*key, modes))
            .collect::<Result<Vec<_>, _>>()
            .map_err(anyhow::Error::msg)?;
        let mut bytes = 0;
        for _ in 0..repeat {
            for (key, encoded) in keys.iter().zip(&encoded) {
                self.write_key(*key, encoded)?;
                bytes += encoded.len();
            }
        }
        let pressed = usize::try_from(repeat).context("the repeat count")? * keys.len();
        Ok(Delivery::Keys { keys: pressed, bytes })
    }

    fn paste(&self, text: &str) -> Result<Delivery> {
        let modes = self.state.lock().unwrap_or_else(PoisonError::into_inner).screen.keyboard_modes();
        let bytes = keyboard::paste(text, modes);
        self.write(&bytes)?;
        Ok(Delivery::Paste { bracketed: modes.bracketed_paste, bytes: bytes.len() })
    }

    fn signal(&self, signal: Signal) -> Result<Delivery> {
        self.ensure_running()?;
        let process_group = self.signaller.signal(signal)?;
        Ok(Delivery::Signal { signal, process_group })
    }

    fn click(&self, click: &Click<'_>) -> Result<Delivery> {
        let (row, col, bytes) = {
            let mut state = self.running()?;
            let reporting = state.screen.mouse_reporting().ok_or_else(no_mouse)?;
            let (row, col) = click_target(&state, click.target)?;
            let mods = click.mods;
            let press = mouse::Event { action: Action::Press(click.button), row, col, mods }
                .encode(reporting)
                .map_err(anyhow::Error::msg)?;
            let release = mouse::Event { action: Action::Release(click.button), row, col, mods }
                .encode(reporting)
                .map_err(anyhow::Error::msg)?;
            state.mouse_held = None;
            (row, col, [press, release].concat().repeat(if click.double { 2 } else { 1 }))
        };
        self.write(&bytes)?;
        Ok(Delivery::Click {
            target: click.target.clone(),
            row,
            col,
            button: click.button,
            double: click.double,
            mods: click.mods,
            bytes: bytes.len(),
        })
    }

    fn mouse(&self, event: MouseEvent) -> Result<Delivery> {
        let bytes = {
            let mut state = self.running()?;
            let reporting = state.screen.mouse_reporting().ok_or_else(no_mouse)?;
            let (row, col) = event.at();
            state.screen.cell_at(row, col).map_err(anyhow::Error::msg)?;
            let action = match event {
                MouseEvent::Move { .. } => Action::Move { held: state.mouse_held },
                MouseEvent::Down { button, .. } => Action::Press(button),
                MouseEvent::Up { button, .. } => Action::Release(button),
                MouseEvent::Wheel { direction, .. } => Action::Wheel(direction),
            };
            let bytes =
                mouse::Event { action, row, col, mods: event.mods() }.encode(reporting).map_err(anyhow::Error::msg)?;
            match event {
                MouseEvent::Down { button, .. } => state.mouse_held = Some(button),
                MouseEvent::Up { .. } => state.mouse_held = None,
                MouseEvent::Move { .. } | MouseEvent::Wheel { .. } => {}
            }
            bytes
        };
        if !bytes.is_empty() {
            self.write(&bytes)?;
        }
        Ok(Delivery::Mouse { event, bytes: bytes.len() })
    }

    fn scroll(&self, direction: Direction, count: u32, keys: bool) -> Result<Delivery> {
        let (bytes, by) = {
            let state = self.running()?;
            if let Some(reporting) = state.screen.mouse_reporting().filter(|_| !keys) {
                let size = state.screen.size();
                let (row, col) = (size.rows.div_ceil(2), size.cols.div_ceil(2));
                let notch = mouse::Event { action: Action::Wheel(direction), row, col, mods: Mods::default() }
                    .encode(reporting)
                    .map_err(anyhow::Error::msg)?;
                (notch, ScrollBy::Wheel)
            } else {
                (page_key(direction, &state)?, ScrollBy::Keys)
            }
        };
        let bytes = bytes.repeat(usize::try_from(count).context("the scroll count")?);
        self.write(&bytes)?;
        Ok(Delivery::Scroll { direction, count, by, bytes: bytes.len() })
    }

    fn focus(&self, focused: bool) -> Result<Delivery> {
        let reported = self.running()?.screen.focus_events();
        let bytes: &[u8] = match (reported, focused) {
            (false, _) => b"",
            (true, true) => b"\x1b[I",
            (true, false) => b"\x1b[O",
        };
        if !bytes.is_empty() {
            self.write(bytes)?;
        }
        Ok(Delivery::Focus { focused, bytes: bytes.len() })
    }

    fn running(&self) -> Result<MutexGuard<'_, State>> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        match state.status {
            Status::Running => Ok(state),
            Status::Exited(code) => Err(self.exited(code).into()),
        }
    }

    fn press(&self, key: Key) -> Result<usize> {
        let modes = self.state.lock().unwrap_or_else(PoisonError::into_inner).screen.keyboard_modes();
        let bytes = keyboard::encode(key, modes).map_err(anyhow::Error::msg)?;
        self.write_key(key, &bytes)?;
        Ok(bytes.len())
    }

    fn pause(&self, delay: Duration) -> Result<()> {
        let until = Instant::now() + delay;
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if let Status::Exited(code) = state.status {
                return Err(self.exited(code).into());
            }
            let now = Instant::now();
            if now >= until {
                return Ok(());
            }
            state = self
                .changed
                .wait_timeout(state, until.saturating_duration_since(now))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    fn write_key(&self, key: Key, bytes: &[u8]) -> Result<()> {
        self.send(bytes, key.code == KeyCode::Enter || ends_a_line(bytes))
    }

    pub(super) fn write(&self, bytes: &[u8]) -> Result<()> {
        self.send(bytes, ends_a_line(bytes))
    }

    fn send(&self, bytes: &[u8], enters: bool) -> Result<()> {
        {
            let mut state = self.running()?;
            if let Some(commands) = &mut state.commands
                && enters
            {
                commands.entered();
            }
        }
        let mut writer = self.writer.lock().unwrap_or_else(PoisonError::into_inner);
        writer.write_all(bytes).and_then(|()| writer.flush()).with_context(|| format!("write to {}", self.program))?;
        Ok(())
    }

    fn ensure_running(&self) -> Result<()> {
        self.running().map(drop)
    }
}

fn page_key(direction: Direction, state: &State) -> Result<Vec<u8>> {
    let code = match direction {
        Direction::Up => KeyCode::PageUp,
        Direction::Down => KeyCode::PageDown,
    };
    let key = Key { code, mods: Mods::default() };
    keyboard::encode(key, state.screen.keyboard_modes()).map_err(anyhow::Error::msg)
}

fn ends_a_line(bytes: &[u8]) -> bool {
    bytes.iter().any(|byte| matches!(byte, b'\r' | b'\n'))
}

fn no_mouse() -> anyhow::Error {
    anyhow!(
        "this app has no mouse support; use keys instead (`agent-terminal snapshot -i` lists the key hints on screen)"
    )
}

fn click_target(state: &State, target: &ClickTarget) -> Result<(u16, u16)> {
    match target {
        ClickTarget::Cell { row, col } => {
            state.screen.cell_at(*row, *col).map_err(anyhow::Error::msg)?;
            Ok((*row, *col))
        }
        ClickTarget::Text(text) => {
            let pattern = wait::compile(text, false).map_err(anyhow::Error::msg)?;
            let found = state.screen.matches(&pattern, 0).into_iter().next();
            let found = found.ok_or_else(|| anyhow!("{text:?} is not on the screen"))?;
            Ok((u16::try_from(found.row).context("the row of the text")?, found.col))
        }
        ClickTarget::Ref(id) => ref_target(state, *id),
    }
}

fn ref_target(state: &State, id: u32) -> Result<(u16, u16)> {
    let missing = || {
        anyhow!("there is no @t{id} in the last `agent-terminal snapshot -i` of this tab; take one and use its refs")
    };
    let listed = state.listed.as_ref().ok_or_else(missing)?;
    let index = usize::try_from(id).ok().and_then(|id| id.checked_sub(1));
    let found = index.and_then(|index| listed.refs.get(index)).ok_or_else(missing)?;
    if !found.still_on(&state.screen.grid()) {
        bail!(
            "@t{id} is from frame {}, and that part of the screen has changed since; take a new `agent-terminal snapshot -i` and use its refs",
            listed.frame
        );
    }
    let row = u16::try_from(found.row() + 1).context("the row of the ref")?;
    let col = u16::try_from(found.col() + 1).context("the column of the ref")?;
    Ok((row, col))
}
