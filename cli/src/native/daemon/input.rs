use std::sync::{Arc, PoisonError};
use std::time::Instant;

use super::refusals::{error_body, human_has_control};
use super::waiting::{condition, wait_for};
use super::{Daemon, Tab};
use crate::native::screenshots;
use crate::native::terminal::Terminal;
use crate::output::call_line;
use crate::protocol::{Delivered, ErrorBody, Reply, Screenshot, Shot, request};

impl Daemon {
    pub(super) fn input(&self, input: request::Deliver, typed: Option<&[String]>) -> Result<Reply, ErrorBody> {
        let (id, Tab { terminal, control }) = self.tab(&input.target)?;
        let (session, tab) = (input.target.session, id.to_string());
        let condition = input.wait.as_ref().map(condition).transpose()?;
        let delivery = {
            let _turn = control.agent().ok_or_else(|| human_has_control(&session, &tab))?;
            terminal.input(&input.input).map_err(|e| error_body(&e.context(format!("{session} {tab}"))))?
        };
        if let Some((wait, condition)) = input.wait.as_ref().zip(condition) {
            let condition = condition.counted_from(Instant::now());
            wait_for(&terminal, &session, &tab, wait, condition, input.timeout_ms)?;
        }
        let screenshot = input
            .screenshot
            .as_ref()
            .map(|shot| self.screenshot(&session, &tab, &terminal, shot, typed))
            .transpose()?
            .map(Box::new);
        let snapshot = input.snapshot.then(|| Box::new(terminal.snapshot(&session, &tab)));
        Ok(Reply::Delivered(Delivered { session, tab, delivery, screenshot, snapshot }))
    }

    pub(super) fn screenshot(
        &self,
        session: &str,
        tab: &str,
        terminal: &Terminal,
        shot: &Shot,
        typed: Option<&[String]>,
    ) -> Result<Screenshot, ErrorBody> {
        let taken =
            screenshots::take(session, tab, terminal, shot, typed.map(call_line)).map_err(|e| error_body(&e))?;
        let folder = self
            .sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(session)
            .map(|entry| Arc::clone(&entry.folder));
        if let Some((entry, folder)) = taken.listed.zip(folder) {
            folder.keep_screenshot(&entry);
        }
        Ok(taken.screenshot)
    }
}
