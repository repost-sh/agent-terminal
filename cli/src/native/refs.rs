use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::{Captures, Regex};

use crate::native::screen::Colour;
use crate::protocol::RefKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Glyph {
    pub text: String,
    pub fg: Colour,
    pub bg: Colour,
    pub emphasis: Emphasis,
    pub decoration: Decoration,
    pub inverse: bool,
    pub wide: bool,
    pub link: Option<Link>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Emphasis {
    pub bold: bool,
    pub italic: bool,
    pub dim: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Decoration {
    pub underline: bool,
    pub strikeout: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Link {
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

pub(crate) struct Grid {
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
pub(crate) struct Area {
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
pub(crate) struct Found {
    kind: RefKind,
    label: String,
    row: usize,
    col: usize,
    area: Area,
    url: Option<String>,
    basis: Span,
    seen: Vec<Glyph>,
}

impl Found {
    fn new(kind: RefKind, label: String, col: usize, area: Area, basis: Span, grid: &Grid) -> Option<Self> {
        let seen = grid.glyphs(area)?;
        Some(Self { kind, label, row: basis.row, col, area, url: None, basis, seen })
    }

    fn on_span(kind: RefKind, label: String, col: usize, span: Span, grid: &Grid) -> Option<Self> {
        Self::new(kind, label, col, span.area(), span, grid)
    }

    pub(crate) fn still_on(&self, grid: &Grid) -> bool {
        grid.glyphs(self.area).as_ref() == Some(&self.seen)
    }

    pub(crate) fn kind(&self) -> RefKind {
        self.kind
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(crate) fn row(&self) -> usize {
        self.row
    }

    pub(crate) fn col(&self) -> usize {
        self.col
    }

    pub(crate) fn area(&self) -> Area {
        self.area
    }

    pub(crate) fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }
}

type Pattern = LazyLock<Result<Regex, regex::Error>>;

fn pattern(compiled: &'static Pattern) -> Result<&'static Regex, String> {
    compiled.as_ref().map_err(|e| format!("a built-in pattern for finding refs does not compile: {e}"))
}

pub(crate) fn find(grid: &Grid) -> Result<Vec<Found>, String> {
    let mut found = panels(grid);
    found.extend(buttons(grid)?);
    found.extend(links(grid));
    let hints: Vec<Found> = hints(grid)?.into_iter().filter(|hint| !overlaps(hint, &found)).collect();
    found.extend(hints);
    let marked = found.clone();
    found.extend(selected(grid).into_iter().filter(|span| !covered(span, &marked, grid)));
    found.extend(input(grid));
    found.sort_by_key(|found| (found.row, found.col, found.kind));
    Ok(found)
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

fn at_is(cells: &[Glyph], col: usize, set: &str) -> bool {
    cells.get(col).is_some_and(|glyph| is(glyph, set))
}

fn panels(grid: &Grid) -> Vec<Found> {
    let mut found = Vec::new();
    for (row, cells) in grid.rows.iter().enumerate() {
        let corners = (0..cells.len()).filter(|&col| at_is(cells, col, TOP_LEFT));
        found.extend(corners.filter_map(|col| panel_at(grid, row, col)));
    }
    found
}

fn panel_at(grid: &Grid, row: usize, col: usize) -> Option<Found> {
    let cells = grid.rows.get(row)?;
    let right = (col + 1..cells.len()).find(|&c| at_is(cells, c, TOP_RIGHT))?;
    let bottom = bottom_of(grid, row, col)?;
    let title_start = (col + 1..right).find(|&c| !at_is(cells, c, HORIZONTAL))?;
    let title_end = (title_start..right).find(|&c| at_is(cells, c, HORIZONTAL)).unwrap_or(right);
    let (label, label_col) = label_in(cells, title_start, title_end - 1)?;
    let area = Area { row, last_row: bottom, col, last_col: right };
    Found::new(RefKind::Panel, label, label_col, area, Span { row, col, last_col: right }, grid)
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
    let span = cells.get(first..=last)?;
    let offset = span.iter().position(|glyph| !glyph.text.trim().is_empty())?;
    let text: String = span.iter().skip(offset).map(|glyph| glyph.text.as_str()).collect();
    Some((text.trim_end().to_string(), first + offset))
}

fn runs<K: PartialEq>(cells: &[Glyph], key: impl Fn(&Glyph) -> K) -> Vec<(usize, usize, K)> {
    let mut runs: Vec<(usize, usize, K)> = Vec::new();
    for (col, glyph) in cells.iter().enumerate() {
        let value = key(glyph);
        match runs.last_mut() {
            Some((_, end, last)) if *last == value => *end = col + 1,
            _ => runs.push((col, col + 1, value)),
        }
    }
    runs
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
        let marked =
            runs(cells, Glyph::backdrop).into_iter().filter(|(col, end, backdrop)| *backdrop != plain && end - col > 1);
        found.extend(marked.filter_map(|(col, end, _)| {
            let (label, label_col) = label_in(cells, col, end - 1)?;
            Found::on_span(RefKind::Selected, label, label_col, Span { row, col, last_col: end - 1 }, grid)
        }));
    }
    found
}

fn covered(span: &Found, others: &[Found], grid: &Grid) -> bool {
    let Some(cells) = grid.rows.get(span.row) else {
        return false;
    };
    (span.area.col..=span.area.last_col)
        .filter(|&col| cells.get(col).is_some_and(|glyph| glyph.char().is_alphanumeric()))
        .all(|col| {
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
    fn col(&self, byte: usize) -> Option<usize> {
        self.cols.get(byte).copied()
    }

    fn segments(&self, separator: &Regex) -> Vec<std::ops::Range<usize>> {
        let mut bounds = vec![0];
        for found in separator.find_iter(&self.text) {
            bounds.extend([found.start(), found.end()]);
        }
        bounds.push(self.text.len());
        bounds
            .as_chunks::<2>()
            .0
            .iter()
            .filter_map(|&[start, end]| {
                let raw = self.text.get(start..end)?;
                let first = start + raw.len() - raw.trim_start().len();
                let last = first + raw.trim().len();
                (last > first).then_some(first..last)
            })
            .collect()
    }
}

static BUTTON: Pattern = LazyLock::new(|| Regex::new(r"\[ +([^\[\]<>│]{1,24}?) +\]|< +([^\[\]<>│]{1,24}?) +>"));

fn buttons(grid: &Grid) -> Result<Vec<Found>, String> {
    let button = pattern(&BUTTON)?;
    let mut found = Vec::new();
    for (row, cells) in grid.rows.iter().enumerate() {
        let line = located(cells);
        found.extend(button.captures_iter(&line.text).filter_map(|captured| button_at(grid, &line, row, &captured)));
    }
    Ok(found)
}

fn button_at(grid: &Grid, line: &Located, row: usize, button: &Captures<'_>) -> Option<Found> {
    let whole = button.get(0)?;
    let label = button.get(1).or_else(|| button.get(2))?;
    if !label.as_str().chars().any(char::is_alphanumeric) {
        return None;
    }
    let span = Span { row, col: line.col(whole.start())?, last_col: line.col(whole.end() - 1)? };
    Found::on_span(RefKind::Button, label.as_str().to_string(), line.col(label.start())?, span, grid)
}

fn links(grid: &Grid) -> Vec<Found> {
    let mut found = Vec::new();
    for (row, cells) in grid.rows.iter().enumerate() {
        let linked = runs(cells, |glyph| glyph.link.clone()).into_iter();
        found.extend(linked.filter_map(|(col, end, link)| {
            let url = link?.url;
            let (label, label_col) = label_in(cells, col, end - 1)?;
            let span = Span { row, col, last_col: end - 1 };
            Found::on_span(RefKind::Link, label, label_col, span, grid).map(|found| Found { url: Some(url), ..found })
        }));
    }
    found
}

fn input(grid: &Grid) -> Option<Found> {
    let (row, col) = grid.cursor?;
    let cells = grid.rows.get(row)?;
    let first = (0..col).rev().find(|&c| at_is(cells, c, VERTICAL)).map_or(0, |border| border + 1);
    let last = (col + 1..cells.len()).find(|&c| at_is(cells, c, VERTICAL)).unwrap_or(cells.len()).checked_sub(1)?;
    let label = label_in(cells, first, last).map_or_else(String::new, |(label, _)| label);
    Found::on_span(RefKind::Input, label, col, Span { row, col: first, last_col: last }, grid)
}

const KEY: &str = r"(?:\^\S|[CMSA]-\S+|(?i:ctrl|alt|shift|cmd|meta|super|opt|option)\+\S+|F\d{1,2}|<[^<>\s]{1,12}>|\[[^\[\]\s]{1,8}\]|(?i:enter|esc|escape|tab|space|spc|ret|return|up|down|left|right|home|end|pgup|pgdn|pageup|pagedown|del|delete|backspace|bksp|ins|insert)|[^\s/]{1,3}(?:/[^\s/]{1,3})+|\S)";
const ACTION: &str = r"[A-Za-z][\w .'&/-]{0,30}";

static SEPARATOR: Pattern = LazyLock::new(|| Regex::new(r"\s*[|│┃║•·]\s*|\s{2,}"));
static HINT: Pattern = LazyLock::new(|| Regex::new(&format!(r"^(?:{ACTION}: {KEY}|{KEY}:? {ACTION})$")));

fn hints(grid: &Grid) -> Result<Vec<Found>, String> {
    let (separator, hint) = (pattern(&SEPARATOR)?, pattern(&HINT)?);
    let mut found = Vec::new();
    for (row, cells) in grid.rows.iter().enumerate() {
        let line = located(cells);
        let row_hints: Vec<Found> = line
            .segments(separator)
            .into_iter()
            .filter_map(|segment| {
                let text = line.text.get(segment.clone()).filter(|text| hint.is_match(text))?;
                let span = Span { row, col: line.col(segment.start)?, last_col: line.col(segment.end - 1)? };
                Found::on_span(RefKind::Hint, text.to_string(), span.col, span, grid)
            })
            .collect();
        if row_hints.len() >= 2 {
            found.extend(row_hints);
        }
    }
    Ok(found)
}
