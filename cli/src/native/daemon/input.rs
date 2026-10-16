use std::sync::{Arc, PoisonError};
use std::time::Instant;

use super::refusals::{error_body, human_has_control};
use super::waiting::{Until, condition};
use super::{Daemon, Tab};
use crate::native::screenshots;
use crate::native::terminal::Labelled;
use crate::output::call_line;
use crate::protocol::{Delivered, ErrorBody, Reply, Screenshot, Shot, request};

pub(super) struct Shooting<'a> {
    pub at: Labelled<'a>,
    pub shot: &'a Shot,
    pub typed: Option<&'a [String]>,
}

impl Daemon {
    pub(super) fn input(&self, input: request::Deliver, typed: Option<&[String]>) -> Result<Reply, ErrorBody> {
        let (id, Tab { terminal, control }) = self.tab(&input.target)?;
        let (session, tab) = (input.target.session, id.to_string());
        let at = Labelled { session: &session, tab: &tab, terminal: &terminal };
        let condition = input.wait.as_ref().map(condition).transpose()?;
        let delivery = {
            let _turn = control.agent().ok_or_else(|| human_has_control(&session, &tab))?;
            terminal.input(&input.input).map_err(|e| error_body(&e.context(format!("{session} {tab}"))))?
        };
        if let Some((wait, condition)) = input.wait.as_ref().zip(condition) {
            let condition = condition.counted_from(Instant::now());
            Until { wait, timeout_ms: input.timeout_ms }.wait_on(&at, condition)?;
        }
        let screenshot = input
            .screenshot
            .as_ref()
            .map(|shot| self.screenshot(&Shooting { at, shot, typed }))
            .transpose()?
            .map(Box::new);
        let snapshot = input.snapshot.then(|| Box::new(at.snapshot()));
        Ok(Reply::Delivered(Delivered { session, tab, delivery, screenshot, snapshot }))
    }

    pub(super) fn screenshot(&self, shooting: &Shooting<'_>) -> Result<Screenshot, ErrorBody> {
        let taken = screenshots::take(&shooting.at, shooting.shot).map_err(|e| error_body(&e))?;
        let folder = self
            .sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(shooting.at.session)
            .map(|entry| Arc::clone(&entry.folder));
        if let Some((mut entry, folder)) = taken.listed.zip(folder) {
            entry.by = shooting.typed.map(call_line);
            folder.keep_screenshot(&entry);
        }
        Ok(taken.screenshot)
    }
}
