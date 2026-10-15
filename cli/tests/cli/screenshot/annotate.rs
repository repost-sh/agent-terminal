use super::picture::{CELL_HEIGHT, CELL_WIDTH, MARGIN, Picture, REF_COLORS, Rgb, picture};
use crate::support::{AgentTerminal, fixture};

#[test]
fn annotate_draws_a_box_and_a_name_on_each_ref_with_a_legend_and_prints_the_refs() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui")]).success();
    let work = at.work_folder();
    let path = work.join("annotated.png");

    let run = at.run(&["screenshot", "--annotate", path.to_str().unwrap()]).success();

    insta::assert_snapshot!(run.screen().replace(&path.display().to_string(), "<path>"), @r#"
    saved <path> (default t1, frame N)
    refs:
      @t1 [panel]    "Files"     rows 1-7
      @t2 [panel]    "Details"   rows 1-7
      @t3 [selected] "alpha.txt" row 2 col 2
      @t4 [hint]     "Move: j/k" row 8 col 1
      @t5 [hint]     "Quit: q"   row 8 col 13
      @t6 [hint]     "Crash: x"  row 8 col 23
    "#);

    let shot = picture(&path);
    let legend_lines = 3;
    assert_eq!((shot.width, shot.height), (50 * 9 + 2 * 10, 8 * 20 + 2 * 10 + 10 + legend_lines * 20));
    let refs: [(u32, u32, u32, u32); 6] =
        [(1, 7, 1, 20), (1, 7, 21, 50), (2, 2, 2, 19), (8, 8, 1, 9), (8, 8, 13, 19), (8, 8, 23, 30)];
    for (index, (cells, color)) in refs.into_iter().zip(REF_COLORS).enumerate() {
        the_ref_has_a_box_in_its_colour(&shot, &format!("@t{}", index + 1), cells, color);
    }
    each_legend_line_starts_with_the_colour_of_its_first_ref(&shot);
}

fn the_ref_has_a_box_in_its_colour(shot: &Picture, name: &str, cells: (u32, u32, u32, u32), color: Rgb) {
    let (first_row, last_row, first_col, last_col) = cells;
    let (left, right) = (MARGIN + (first_col - 1) * CELL_WIDTH, MARGIN + last_col * CELL_WIDTH - 1);
    let (top, bottom) = (MARGIN + (first_row - 1) * CELL_HEIGHT, MARGIN + last_row * CELL_HEIGHT - 1);
    let (middle_x, middle_y) = (u32::midpoint(left, right), u32::midpoint(top, bottom));
    assert_eq!(shot.at(left, middle_y), color, "the left side of {name}'s box");
    assert_eq!(shot.at(right, middle_y), color, "the right side of {name}'s box");
    assert_eq!(shot.at(middle_x, bottom), color, "the bottom of {name}'s box");
}

fn each_legend_line_starts_with_the_colour_of_its_first_ref(shot: &Picture) {
    let legend_top = 8 * 20 + 2 * 10;
    for (line, first_ref) in (0u32..).zip([0, 2, 4]) {
        let y = legend_top + line * CELL_HEIGHT;
        assert_eq!(
            shot.at(MARGIN, y),
            REF_COLORS[first_ref],
            "legend line {} starts with @t{}",
            line + 1,
            first_ref + 1
        );
    }
}

#[test]
fn an_annotated_svg_draws_the_legend_as_text() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui")]).success();
    let path = at.work_folder().join("annotated.svg");
    at.run(&["screenshot", "--annotate", path.to_str().unwrap()]).success();
    let drawing = std::fs::read_to_string(&path).unwrap();
    let legend: Vec<&str> =
        drawing.lines().filter(|line| line.starts_with("<rect x=") && line.contains("xml:space")).collect();
    assert_eq!(
        legend.first().copied(),
        Some(
            "<rect x=\"10\" y=\"180\" width=\"27\" height=\"20\" fill=\"#fb923c\"/><text x=\"10\" y=\"195\" fill=\"#111113\">@t1</text>\
             <text x=\"46\" y=\"195\" fill=\"#d4d4d8\" xml:space=\"preserve\">panel &quot;Files&quot;</text>"
        )
    );
    let entries: Vec<(&str, &str)> = legend
        .iter()
        .map(|line| {
            let tag = line.split(">@").nth(1).and_then(|rest| rest.split('<').next()).unwrap_or_default();
            let rest =
                line.rsplit("preserve\">").next().and_then(|rest| rest.strip_suffix("</text>")).unwrap_or_default();
            (tag, rest)
        })
        .collect();
    assert_eq!(
        entries,
        [
            ("t1", "panel &quot;Files&quot;"),
            ("t2", "panel &quot;Details&quot;"),
            ("t3", "selected &quot;alpha.txt&quot;"),
            ("t4", "hint &quot;Move: j/k&quot;"),
            ("t5", "hint &quot;Quit: q&quot;"),
            ("t6", "hint &quot;Crash: x&quot;")
        ]
    );
}
