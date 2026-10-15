use std::sync::PoisonError;

use anyhow::Result;

use super::Terminal;
use crate::native::commands::{Commands, Submitted};
use crate::protocol::{CommandRecord, Shell};

impl Terminal {
    pub(crate) fn shell(&self) -> Option<Shell> {
        self.shell
    }

    pub(crate) fn submit(&self, line: &str) -> Result<Option<Submitted>> {
        let _turn = self.input_turn.lock().unwrap_or_else(PoisonError::into_inner);
        let submitted = {
            let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            match &state.commands {
                Some(commands) if commands.prompt().ready => commands.submitted(),
                _ => return Ok(None),
            }
        };
        let mut bytes = line.as_bytes().to_vec();
        bytes.push(b'\r');
        self.write(&bytes)?;
        Ok(Some(submitted))
    }

    pub(crate) fn running_command(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .commands
            .as_ref()
            .and_then(|commands| commands.running().map(str::to_string))
    }

    pub(crate) fn last_exit_code(&self) -> Option<i32> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner).commands.as_ref().and_then(Commands::last_exit_code)
    }

    pub(crate) fn commands(&self) -> Option<Vec<CommandRecord>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner).commands.as_ref().map(Commands::records)
    }

    pub(crate) fn command(&self, id: u32) -> Option<(CommandRecord, String)> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let (record, from, to) = state.commands.as_ref()?.record(id)?;
        Some((record, state.log.text_between(from, to)))
    }

    pub(crate) fn styled_command(&self, id: u32) -> Option<(CommandRecord, String)> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let (record, from, to) = state.commands.as_ref()?.record(id)?;
        Some((record, state.log.styled_between(from, to)))
    }
}
