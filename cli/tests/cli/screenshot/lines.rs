use super::picture::{BACKGROUND, Cell, Picture, picture};
use crate::support::{AgentTerminal, Screen, Shown};

const LINES: &str = "\x1b[?25l┄ ┆ ┈ ┊ ╌ ╎ ┅ ┇\n╟ ╓ ╢ ╖ ╤ ╕ ╧ ╛";

const MIDDLE_ROW: u32 = 9;
const MIDDLE_COL: u32 = 4;

fn lines_picture(at: &AgentTerminal) -> Picture {
    let screen = Screen::open(at, &Shown { size: "16x2", bytes: LINES, wait: "╛" });
    let path = screen.file("lines.png");
    at.run(&["screenshot", &path.to_string_lossy()]).success();
    picture(&path)
}

fn inked(cell: &Cell<'_>, x: u32, y: u32) -> bool {
    cell.patch(x..x + 1, y..y + 1).inked(BACKGROUND)
}

fn dashes(cell: &Cell<'_>, across: bool) -> usize {
    let along: Vec<bool> = if across {
        (0..9).map(|x| inked(cell, x, MIDDLE_ROW)).collect()
    } else {
        (0..20).map(|y| inked(cell, MIDDLE_COL, y)).collect()
    };
    along.windows(2).filter(|pair| pair == &[false, true]).count() + usize::from(along.first() == Some(&true))
}

#[test]
fn a_dashed_line_breaks_into_its_dashes_across_or_down_the_cell() {
    let at = AgentTerminal::new();
    let shot = lines_picture(&at);
    let counted: Vec<usize> =
        [(1, true), (3, false), (5, true), (7, false), (9, true), (11, false), (13, true), (15, false)]
            .into_iter()
            .map(|(col, across)| dashes(&shot.cell(1, col), across))
            .collect();
    assert_eq!(counted, [3, 3, 4, 4, 2, 2, 3, 3], "┄ ┆ ┈ ┊ ╌ ╎ ┅ ┇");
}

struct Join {
    what: &'static str,
    col: u32,
    across: bool,
    span: std::ops::Range<u32>,
    pixels: &'static [bool],
}

const JOINS: [Join; 8] = [
    Join {
        what: "╟ starts at the right line, leaving the gap",
        col: 1,
        across: true,
        span: 3..9,
        pixels: &[true, false, true, true, true, true],
    },
    Join {
        what: "╓ starts at the left line",
        col: 3,
        across: true,
        span: 2..9,
        pixels: &[false, true, true, true, true, true, true],
    },
    Join {
        what: "╢ ends at the left line, leaving the gap",
        col: 5,
        across: true,
        span: 0..6,
        pixels: &[true, true, true, true, false, true],
    },
    Join {
        what: "╖ ends at the right line",
        col: 7,
        across: true,
        span: 0..7,
        pixels: &[true, true, true, true, true, true, false],
    },
    Join {
        what: "╤ starts at the lower line, leaving the gap",
        col: 9,
        across: false,
        span: 8..12,
        pixels: &[true, false, true, true],
    },
    Join { what: "╕ starts at the upper line", col: 11, across: false, span: 7..10, pixels: &[false, true, true] },
    Join {
        what: "╧ ends at the upper line, leaving the gap",
        col: 13,
        across: false,
        span: 7..11,
        pixels: &[true, true, false, true],
    },
    Join { what: "╛ ends at the lower line", col: 15, across: false, span: 9..12, pixels: &[true, true, false] },
];

#[test]
fn a_single_line_meets_a_double_line_at_the_side_it_joins() {
    let at = AgentTerminal::new();
    let shot = lines_picture(&at);
    let drawn: Vec<(&str, Vec<bool>)> = JOINS
        .iter()
        .map(|join| {
            let cell = shot.cell(2, join.col);
            let pixels = join
                .span
                .clone()
                .map(|at| if join.across { inked(&cell, at, MIDDLE_ROW) } else { inked(&cell, MIDDLE_COL, at) });
            (join.what, pixels.collect())
        })
        .collect();
    let expected: Vec<(&str, Vec<bool>)> = JOINS.iter().map(|join| (join.what, join.pixels.to_vec())).collect();
    assert_eq!(drawn, expected);
}
