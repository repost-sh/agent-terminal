use super::Daemon;
use crate::native::terminal::{Labelled, Status};
use crate::native::wait::Pattern;
use crate::protocol::{
    Checked, ErrorBody, Expect, Reply, RowRange, Snapshot, SnapshotOptions, Style, TextMatch, request,
};

impl Daemon {
    pub(super) fn expect(&self, expect: &request::Expect) -> Result<Reply, ErrorBody> {
        let (id, terminal) = self.target(&expect.target)?;
        let tab = id.to_string();
        let checking = Checking { at: Labelled { session: &expect.target.session, tab: &tab, terminal: &terminal } };
        let fact = match &expect.expect {
            Expect::Text(text) => checking.on_screen(text)?,
            Expect::Gone(text) => checking.gone(text)?,
            Expect::ExitCode { code } => checking.exited_with(*code)?,
            Expect::Cell { row, col, style } => checking.cell(*row, *col, style)?,
        };
        Ok(Reply::Checked(Checked { session: expect.target.session.clone(), tab, fact }))
    }
}

struct Checking<'a> {
    at: Labelled<'a>,
}

impl Checking<'_> {
    fn place(&self) -> String {
        format!("{} {}", self.at.session, self.at.tab)
    }

    fn excerpt(&self, rows: Option<RowRange>) -> Result<Snapshot, ErrorBody> {
        let options = SnapshotOptions { rows, ..SnapshotOptions::default() };
        self.at.snapshot_with(&options).map_err(ErrorBody::failure)
    }

    fn on_screen(&self, text: &TextMatch) -> Result<String, ErrorBody> {
        if !Pattern::new(text).map_err(ErrorBody::failure)?.found(&self.at.terminal.lines()) {
            return Err(assertion(format!("{text} is not on screen in {}", self.place()), self.excerpt(None)?));
        }
        Ok(format!("{text} is on screen"))
    }

    fn gone(&self, text: &TextMatch) -> Result<String, ErrorBody> {
        let rows = Pattern::new(text).map_err(ErrorBody::failure)?.rows_found(&self.at.terminal.lines());
        if let (Some(&first), Some(&last)) = (rows.first(), rows.last()) {
            let which = if first == last { format!("row {first}") } else { format!("rows {first}-{last}") };
            let shown = self.excerpt(Some(RowRange { first, last }))?;
            return Err(assertion(format!("{text} is on screen in {}, in {which}", self.place()), shown));
        }
        Ok(format!("{text} is not on screen"))
    }

    fn exited_with(&self, code: i32) -> Result<String, ErrorBody> {
        match self.at.terminal.status() {
            Status::Exited(Some(exited)) if exited == code => {
                Ok(format!("{} exited with code {code}", self.at.terminal.program()))
            }
            Status::Exited(_) => Err(exited_otherwise(code, self.at.snapshot())),
            Status::Running => {
                let message = format!(
                    "{} in {} is still running, so it has not exited with code {code}",
                    self.at.terminal.program(),
                    self.place()
                );
                Err(assertion(message, self.excerpt(None)?))
            }
        }
    }

    fn cell(&self, row: u16, col: u16, style: &[Style]) -> Result<String, ErrorBody> {
        let cell = self.at.terminal.cell(row, col).map_err(ErrorBody::failure)?;
        let missing: Vec<&Style> = style.iter().filter(|style| !style.holds(&cell)).collect();
        if !missing.is_empty() {
            let message = format!("cell {row},{col} in {} is not {}: it is {cell}", self.place(), listed(&missing));
            return Err(assertion(message, self.excerpt(Some(RowRange { first: row, last: row }))?));
        }
        Ok(format!("cell {row},{col} is {}", listed(&style.iter().collect::<Vec<_>>())))
    }
}

fn assertion(message: String, snapshot: Snapshot) -> ErrorBody {
    ErrorBody::new(crate::exit::ASSERTION, message).with_screen(snapshot)
}

fn listed(styles: &[&Style]) -> String {
    styles.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
}

pub(super) fn exited_otherwise(expected: i32, snapshot: Snapshot) -> ErrorBody {
    let how = snapshot.exit_code.map_or_else(|| "without an exit code".to_string(), |code| format!("with code {code}"));
    let message = format!("{} in {} {} exited {how}, not {expected}", snapshot.program, snapshot.session, snapshot.tab);
    assertion(message, snapshot)
}
