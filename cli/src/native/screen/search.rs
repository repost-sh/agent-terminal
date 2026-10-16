use super::Screen;
use crate::protocol::{Cell, Match};

impl Screen {
    pub(crate) fn cell_at(&self, row: u16, col: u16) -> Result<Cell, String> {
        self.cell(row, col).ok_or_else(|| {
            let size = self.view().size;
            format!("cell {row},{col} is outside the {}x{} screen", size.cols, size.rows)
        })
    }

    pub(crate) fn matches(&self, pattern: &regex::Regex, first: i32) -> Vec<Match> {
        (first..i32::from(self.size().rows)).flat_map(|line| self.matches_on(pattern, line)).collect()
    }

    fn matches_on(&self, pattern: &regex::Regex, line: i32) -> Vec<Match> {
        let (row, columns) = self.located_row(line);
        let found = pattern.find_iter(&row).filter(|m| !m.is_empty());
        found
            .filter_map(|found| {
                Some(Match {
                    row: if line < 0 { line } else { line + 1 },
                    col: columns.get(found.start())? + 1,
                    text: found.as_str().to_string(),
                })
            })
            .collect()
    }
}
