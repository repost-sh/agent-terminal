use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::native::screen::Colour;
use crate::protocol::RefKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub text: String,
    pub fg: Colour,
    pub bg: Colour,
    pub inverse: bool,
    pub bold: bool,
    pub underline: bool,
    pub italic: bool,
    pub dim: bool,
    pub strikeout: bool,
    pub wide: bool,
    pub link: Option<Link>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub id: String,
    pub url: String,
}

impl Glyph {
    fn char(&self) -> char {
        self.text.chars().next().unwrap_or(' ')
    }

    fn backdrop(&self) -> Backdrop {
        match (self.inverse, self.fg) {
            (false, _) => Backdrop::Colour(self.bg),
            (true, Colour::Default) => Backdrop::Foreground,
            (true, fg) => Backdrop::Colour(fg),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Backdrop {
    Colour(Colour),
    Foreground,
}

pub struct Grid {
    pub rows: Vec<Vec<Glyph>>,
    pub cursor: Option<(usize, usize)>,
}

impl Grid {
    fn glyphs(&self, area: Area) -> Option<Vec<Glyph>> {
        let rows = self.rows.get(area.row..=area.last_row)?;
        let cells = rows.iter().map(|row| row.get(area.col..=area.last_col)).collect::<Option<Vec<_>>>()?;
        Some(cells.concat())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub row: usize,
    pub last_row: usize,
    pub col: usize,
    pub last_col: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Span {
    row: usize,
    col: usize,
    last_col: usize,
}

impl Span {
    fn area(self) -> Area {
        Area { row: self.row, last_row: self.row, col: self.col, last_col: self.last_col }
    }
}

#[derive(Clone, Debug)]
pub struct Found {
    pub kind: RefKind,
    pub label: String,
    pub row: usize,
    pub col: usize,
    pub area: Area,
    pub url: Option<String>,
    basis: Span,
    seen: Vec<Glyph>,
}

impl Found {
    fn new(kind: RefKind, label: String, col: usize, area: Area, basis: Span, grid: &Grid) -> Self {
        let seen = grid.glyphs(area).expect("a ref covers cells of the grid it was found on");
        Self { kind, label, row: basis.row, col, area, url: None, basis, seen }
    }

    fn on_span(kind: RefKind, label: String, col: usize, span: Span, grid: &Grid) -> Self {
        Self::new(kind, label, col, span.area(), span, grid)
    }

    pub fn still_on(&self, grid: &Grid) -> bool {
        grid.glyphs(self.area).as_ref() == Some(&self.seen)
    }
}

pub fn find(grid: &Grid) -> Vec<Found> {
    let mut found = panels(grid);
    found.extend(buttons(grid));
    found.extend(links(grid));
    let hints: Vec<Found> = hints(grid).into_iter().filter(|hint| !overlaps(hint, &found)).collect();
    found.extend(hints);
    let marked = found.clone();
    found.extend(selected(grid).into_iter().filter(|span| !covered(span, &marked, grid)));
    found.extend(input(grid));
    found.sort_by_key(|found| (found.row, found.col, found.kind));
    found
}

fn overlaps(span: &Found, others: &[Found]) -> bool {
    others
        .iter()
        .map(|other| other.basis)
        .any(|basis| basis.row == span.row && basis.col <= span.area.last_col && span.area.col <= basis.last_col)
}

const TOP_LEFT: &str = "┌╭╔┏╒╓┍┎";
const TOP_RIGHT: &str = "┐╮╗┓╕╖┑┒";
const BOTTOM_LEFT: &str = "└╰╚┗╘╙┕┖";
const HORIZONTAL: &str = "─━═╌╍┄┅┈┉┬┳╦╤╥┯┰";
const VERTICAL: &str = "│┃║╎╏┆┇┊┋├┣╠╞╟┝┠";

fn is(glyph: &Glyph, set: &str) -> bool {
    set.contains(glyph.char())
}

fn panels(grid: &Grid) -> Vec<Found> {
    let mut found = Vec::new();
    for (row, cells) in grid.rows.iter().enumerate() {
        for col in (0..cells.len()).filter(|&col| is(&cells[col], TOP_LEFT)) {
            let Some(right) = (col + 1..cells.len()).find(|&c| is(&cells[c], TOP_RIGHT)) else { continue };
            let Some(bottom) = bottom_of(grid, row, col) else { continue };
            let title_start = (col + 1..right).find(|&c| !is(&cells[c], HORIZONTAL));
            let Some(title_start) = title_start else { continue };
            let title_end = (title_start..right).find(|&c| is(&cells[c], HORIZONTAL)).unwrap_or(right);
            let Some((label, label_col)) = label_in(cells, title_start, title_end - 1) else { continue };
            let area = Area { row, last_row: bottom, col, last_col: right };
            found.push(Found::new(RefKind::Panel, label, label_col, area, Span { row, col, last_col: right }, grid));
        }
    }
    found
}

fn bottom_of(grid: &Grid, top: usize, col: usize) -> Option<usize> {
    let mut bottom = None;
    for (row, cells) in grid.rows.iter().enumerate().skip(top + 1) {
        match cells.get(col) {
            Some(glyph) if is(glyph, BOTTOM_LEFT) => return Some(row),
            Some(glyph) if is(glyph, VERTICAL) => bottom = Some(row),
            _ => break,
        }
    }
    bottom
}

fn label_in(cells: &[Glyph], first: usize, last: usize) -> Option<(String, usize)> {
    let start = (first..=last).find(|&c| !cells[c].text.trim().is_empty())?;
    let text: String = cells[start..=last].iter().map(|glyph| glyph.text.as_str()).collect();
    Some((text.trim_end().to_string(), start))
}

fn selected(grid: &Grid) -> Vec<Found> {
    let mut counts: BTreeMap<Backdrop, usize> = BTreeMap::new();
    for glyph in grid.rows.iter().flatten() {
        *counts.entry(glyph.backdrop()).or_default() += 1;
    }
    let Some(plain) = counts.iter().max_by_key(|(_, count)| **count).map(|(backdrop, _)| *backdrop) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for (row, cells) in grid.rows.iter().enumerate() {
        let mut col = 0;
        while col < cells.len() {
            let backdrop = cells[col].backdrop();
            let end = (col..cells.len()).find(|&c| cells[c].backdrop() != backdrop).unwrap_or(cells.len());
            if backdrop != plain
                && end - col > 1
                && let Some((label, label_col)) = label_in(cells, col, end - 1)
            {
                let span = Span { row, col, last_col: end - 1 };
                found.push(Found::on_span(RefKind::Selected, label, label_col, span, grid));
            }
            col = end;
        }
    }
    found
}

fn covered(span: &Found, others: &[Found], grid: &Grid) -> bool {
    let cells = &grid.rows[span.row];
    (span.area.col..=span.area.last_col).filter(|&col| cells[col].char().is_alphanumeric()).all(|col| {
        others.iter().any(|other| {
            let area = other.basis.area();
            (area.row..=area.last_row).contains(&span.row) && (area.col..=area.last_col).contains(&col)
        })
    })
}

struct Located {
    text: String,
    cols: Vec<usize>,
}

fn located(cells: &[Glyph]) -> Located {
    let mut text = String::new();
    let mut cols = Vec::new();
    for (col, glyph) in cells.iter().enumerate() {
        text.push_str(&glyph.text);
        cols.resize(text.len(), col);
    }
    Located { text, cols }
}

impl Located {
    fn segments(&self) -> Vec<std::ops::Range<usize>> {
        let mut bounds = vec![0];
        for separator in SEPARATOR.find_iter(&self.text) {
            bounds.extend([separator.start(), separator.end()]);
        }
        bounds.push(self.text.len());
        bounds
            .chunks(2)
            .filter_map(|bound| {
                let raw = &self.text[bound[0]..bound[1]];
                let start = bound[0] + raw.len() - raw.trim_start().len();
                let end = start + raw.trim().len();
                (end > start).then_some(start..end)
            })
            .collect()
    }
}

static BUTTON: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[ +([^\[\]<>│]{1,24}?) +\]|< +([^\[\]<>│]{1,24}?) +>").expect("button regex"));

fn buttons(grid: &Grid) -> Vec<Found> {
    let mut found = Vec::new();
    for (row, cells) in grid.rows.iter().enumerate() {
        let line = located(cells);
        for button in BUTTON.captures_iter(&line.text) {
            let (Some(whole), Some(label)) = (button.get(0), button.get(1).or(button.get(2))) else { continue };
            if !label.as_str().chars().any(char::is_alphanumeric) {
                continue;
            }
            let span = Span { row, col: line.cols[whole.start()], last_col: line.cols[whole.end() - 1] };
            found.push(Found::on_span(
                RefKind::Button,
                label.as_str().to_string(),
                line.cols[label.start()],
                span,
                grid,
            ));
        }
    }
    found
}

fn links(grid: &Grid) -> Vec<Found> {
    let mut found = Vec::new();
    for (row, cells) in grid.rows.iter().enumerate() {
        let mut col = 0;
        while col < cells.len() {
            let link = &cells[col].link;
            let end = (col..cells.len()).find(|&c| cells[c].link != *link).unwrap_or(cells.len());
            if let Some(link) = link
                && let Some((label, label_col)) = label_in(cells, col, end - 1)
            {
                let mut ref_found =
                    Found::on_span(RefKind::Link, label, label_col, Span { row, col, last_col: end - 1 }, grid);
                ref_found.url = Some(link.url.clone());
                found.push(ref_found);
            }
            col = end;
        }
    }
    found
}

fn input(grid: &Grid) -> Option<Found> {
    let (row, col) = grid.cursor?;
    let cells = grid.rows.get(row)?;
    let first = (0..col).rev().find(|&c| is(&cells[c], VERTICAL)).map_or(0, |border| border + 1);
    let last = (col + 1..cells.len()).find(|&c| is(&cells[c], VERTICAL)).unwrap_or(cells.len()) - 1;
    let label = label_in(cells, first, last).map_or_else(String::new, |(label, _)| label);
    Some(Found::on_span(RefKind::Input, label, col, Span { row, col: first, last_col: last }, grid))
}

const KEY: &str = r"(?:\^\S|[CMSA]-\S+|(?i:ctrl|alt|shift|cmd|meta|super|opt|option)\+\S+|F\d{1,2}|<[^<>\s]{1,12}>|\[[^\[\]\s]{1,8}\]|(?i:enter|esc|escape|tab|space|spc|ret|return|up|down|left|right|home|end|pgup|pgdn|pageup|pagedown|del|delete|backspace|bksp|ins|insert)|[^\s/]{1,3}(?:/[^\s/]{1,3})+|\S)";
const ACTION: &str = r"[A-Za-z][\w .'&/-]{0,30}";

static SEPARATOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*[|│┃║•·]\s*|\s{2,}").expect("separator regex"));
static HINT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^(?:{ACTION}: {KEY}|{KEY}:? {ACTION})$")).expect("hint regex"));

fn hints(grid: &Grid) -> Vec<Found> {
    let mut found = Vec::new();
    for (row, cells) in grid.rows.iter().enumerate() {
        let line = located(cells);
        let row_hints: Vec<Found> = line
            .segments()
            .into_iter()
            .filter(|segment| HINT.is_match(&line.text[segment.clone()]))
            .map(|segment| {
                let span = Span { row, col: line.cols[segment.start], last_col: line.cols[segment.end - 1] };
                Found::on_span(RefKind::Hint, line.text[segment].to_string(), span.col, span, grid)
            })
            .collect();
        if row_hints.len() >= 2 {
            found.extend(row_hints);
        }
    }
    found
}
