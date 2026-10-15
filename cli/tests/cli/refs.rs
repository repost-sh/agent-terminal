use crate::support::{AgentTerminal, fixture};

#[test]
fn snapshot_i_lists_refs_for_panels_the_selected_row_and_key_hints() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui")]).success();
    insta::assert_snapshot!(at.run(&["snapshot", "-i"]).success().screen(), @r#"
    default t1 · fixture-tui · 50x8 · alt-screen · cursor hidden · frame N
    1│┌Files─────────────┐┌Details─────────────────────┐
    2││alpha.txt         ││Selected: alpha.txt         │
    3││beta.txt          ││Size: 50x8                  │
    4││gamma.txt         ││                            │
    5││delta.txt         ││                            │
    6││                  ││                            │
    7│└──────────────────┘└────────────────────────────┘
    8│Move: j/k | Quit: q | Crash: x
    refs:
      @t1 [panel]    "Files"     rows 1-7
      @t2 [panel]    "Details"   rows 1-7
      @t3 [selected] "alpha.txt" row 2 col 2
      @t4 [hint]     "Move: j/k" row 8 col 1
      @t5 [hint]     "Quit: q"   row 8 col 13
      @t6 [hint]     "Crash: x"  row 8 col 23
    "#);
}

#[test]
fn snapshot_i_json_gives_each_ref_its_place_area_and_frame() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "50x8", "--wait", "Quit: q", &fixture("fixture-tui")]).success();
    let json = at.run(&["--json", "snapshot", "-i"]).success().json();
    let frame = json["frame"].as_u64().expect("a frame number");
    assert_eq!(
        json["refs"][0],
        serde_json::json!({
            "ref": "@t1", "kind": "panel", "label": "Files",
            "row": 1, "col": 2, "rows": [1, 7], "cols": [1, 20], "frame": frame
        })
    );
    assert_eq!(
        json["refs"][2],
        serde_json::json!({
            "ref": "@t3", "kind": "selected", "label": "alpha.txt",
            "row": 2, "col": 2, "rows": [2, 2], "cols": [2, 19], "frame": frame
        })
    );
    assert!(at.run(&["--json", "snapshot"]).success().json().get("refs").is_none());
}

struct Screen {
    _dir: tempfile::TempDir,
}

impl Screen {
    fn open(at: &AgentTerminal, size: &str, bytes: &str, wait: &str) -> Self {
        let dir = crate::support::scratch("screen-");
        let path = dir.path().join("screen");
        std::fs::write(&path, bytes).expect("write the screen");
        let path = path.to_string_lossy().into_owned();
        at.run(&["open", "--size", size, "--wait", wait, "sh", "-c", "cat \"$1\"; read _", "sh", &path]).success();
        Self { _dir: dir }
    }
}

#[test]
fn box_titles_are_panels() {
    let at = AgentTerminal::new();
    let _screen = Screen::open(
        &at,
        "30x7",
        "\x1b[?25l\
         ┌─ Status ──┐ ╭─ Log ─────╮\n\
         │ ok        │ │ started   │\n\
         └───────────┘ ╰───────────╯\n\
         ╔═ Help ═══╗  ┌──────┐\n\
         ║ press ?  ║  │ none │\n\
         ╚══════════╝  └──────┘",
        "└──────┘",
    );
    insta::assert_snapshot!(at.run(&["snapshot", "-i"]).success().screen(), @r#"
    default t1 · sh · 30x7 · cursor hidden · frame N
    1│┌─ Status ──┐ ╭─ Log ─────╮
    2││ ok        │ │ started   │
    3│└───────────┘ ╰───────────╯
    4│╔═ Help ═══╗  ┌──────┐
    5│║ press ?  ║  │ none │
    6│╚══════════╝  └──────┘
    …│ (row 7 blank)
    refs:
      @t1 [panel] "Status" rows 1-3
      @t2 [panel] "Log"    rows 1-3
      @t3 [panel] "Help"   rows 4-6
    "#);
}

#[test]
fn inverse_and_highlighted_spans_are_selected() {
    let at = AgentTerminal::new();
    let _screen = Screen::open(
        &at,
        "30x6",
        "\x1b[?25l\
         Files\n\
         \x1b[7m  notes.txt         \x1b[0m\n\
         \x20 todo.txt\n\
         \x1b[44m  Build  \x1b[0m  \x1b[1mbold only\x1b[0m\n\
         x\x1b[7my\x1b[0m end",
        "y end",
    );
    insta::assert_snapshot!(at.run(&["snapshot", "-i"]).success().screen(), @r#"
    default t1 · sh · 30x6 · cursor hidden · frame N
    1│Files
    2│  notes.txt
    3│  todo.txt
    4│  Build    bold only
    5│xy end
    …│ (row 6 blank)
    refs:
      @t1 [selected] "notes.txt" row 2 col 3
      @t2 [selected] "Build"     row 4 col 3
    "#);
    let refs = at.run(&["--json", "snapshot", "-i"]).success().json()["refs"].clone();
    assert_eq!((&refs[0]["cols"], &refs[1]["cols"]), (&serde_json::json!([1, 20]), &serde_json::json!([1, 9])));
}

#[test]
fn a_background_painted_everywhere_is_not_a_selection() {
    let at = AgentTerminal::new();
    let _screen =
        Screen::open(&at, "20x3", "\x1b[?25l\x1b[44m\x1b[2J\x1b[H  one\n\x1b[7m two \x1b[27m\nthree", "three");
    insta::assert_snapshot!(at.run(&["snapshot", "-i"]).success().screen(), @r#"
    default t1 · sh · 20x3 · cursor hidden · frame N
    1│  one
    2│ two
    3│three
    refs:
      @t1 [selected] "two" row 2 col 2
    "#);
}

#[test]
fn padded_bracket_and_angle_labels_are_buttons() {
    let at = AgentTerminal::new();
    let _screen = Screen::open(
        &at,
        "40x4",
        "\x1b[?25l\
         Save changes?\n\
         \x20 [ OK ]  [ Cancel ]  < Yes >  <  No  >\n\
         status [x] <space> [INFO] [ ]",
        "[INFO] [ ]",
    );
    insta::assert_snapshot!(at.run(&["snapshot", "-i"]).success().screen(), @r#"
    default t1 · sh · 40x4 · cursor hidden · frame N
    1│Save changes?
    2│  [ OK ]  [ Cancel ]  < Yes >  <  No  >
    3│status [x] <space> [INFO] [ ]
    …│ (row 4 blank)
    refs:
      @t1 [button] "OK"     row 2 col 5
      @t2 [button] "Cancel" row 2 col 13
      @t3 [button] "Yes"    row 2 col 25
      @t4 [button] "No"     row 2 col 35
    "#);
    let refs = at.run(&["--json", "snapshot", "-i"]).success().json()["refs"].clone();
    assert_eq!(refs[0]["cols"], serde_json::json!([3, 8]));
}

#[test]
fn osc_8_hyperlinks_are_links_with_their_url() {
    let at = AgentTerminal::new();
    let _screen = Screen::open(
        &at,
        "30x3",
        "\x1b[?25lSee \x1b]8;;https://example.com/docs\x1b\\the docs\x1b]8;;\x1b\\ or \
         \x1b]8;id=x;https://example.com/faq\x1b\\FAQ\x1b]8;;\x1b\\.",
        "FAQ.",
    );
    insta::assert_snapshot!(at.run(&["snapshot", "-i"]).success().screen(), @r#"
    default t1 · sh · 30x3 · cursor hidden · frame N
    1│See the docs or FAQ.
    …│ (rows 2-3 blank)
    refs:
      @t1 [link] "the docs" row 1 col 5
      @t2 [link] "FAQ"      row 1 col 17
    "#);
    let refs = at.run(&["--json", "snapshot", "-i"]).success().json()["refs"].clone();
    assert_eq!(refs[0]["url"], "https://example.com/docs");
    assert_eq!(refs[1]["url"], "https://example.com/faq");
    assert_eq!(refs[0]["cols"], serde_json::json!([5, 12]));
}

#[test]
fn rows_of_key_bindings_are_hints() {
    let at = AgentTerminal::new();
    let _screen = Screen::open(
        &at,
        "60x6",
        "\x1b[?25l\
         Error: file not found\n\
         Size: 80x24  Mode: insert\n\
         ^G Help    ^O Write Out    ^X Exit\n\
         ↑/k up • ↓/j down • q quit\n\
         Stage: <space> | Commit: c | Keybindings: ?",
        "Keybindings: ?",
    );
    insta::assert_snapshot!(at.run(&["snapshot", "-i"]).success().screen(), @r#"
    default t1 · sh · 60x6 · cursor hidden · frame N
    1│Error: file not found
    2│Size: 80x24  Mode: insert
    3│^G Help    ^O Write Out    ^X Exit
    4│↑/k up • ↓/j down • q quit
    5│Stage: <space> | Commit: c | Keybindings: ?
    …│ (row 6 blank)
    refs:
      @t1 [hint] "^G Help"        row 3 col 1
      @t2 [hint] "^O Write Out"   row 3 col 12
      @t3 [hint] "^X Exit"        row 3 col 28
      @t4 [hint] "↑/k up"         row 4 col 1
      @t5 [hint] "↓/j down"       row 4 col 10
      @t6 [hint] "q quit"         row 4 col 21
      @t7 [hint] "Stage: <space>" row 5 col 1
      @t8 [hint] "Commit: c"      row 5 col 18
      @t9 [hint] "Keybindings: ?" row 5 col 30
    "#);
}

#[test]
fn the_text_around_a_visible_cursor_is_the_input() {
    let at = AgentTerminal::new();
    let _screen = Screen::open(
        &at,
        "30x4",
        "┌─ Login ────────────┐\n\
         │ Name: john         │\n\
         └────────────────────┘\x1b[2;13H",
        "Name: john",
    );
    at.run(&["wait", "--cursor", "2,13"]).success();
    insta::assert_snapshot!(at.run(&["snapshot", "-i"]).success().screen(), @r#"
    default t1 · sh · 30x4 · cursor 2,13 · frame N
    1│┌─ Login ────────────┐
    2││ Name: john         │
    3│└────────────────────┘
    …│ (row 4 blank)
    refs:
      @t1 [panel] "Login"      rows 1-3
      @t2 [input] "Name: john" row 2 col 13
    "#);
}

#[test]
fn a_screen_with_nothing_to_act_on_has_no_refs() {
    let at = AgentTerminal::new();
    let _screen = Screen::open(&at, "20x2", "\x1b[?25ljust text", "just text");
    insta::assert_snapshot!(at.run(&["snapshot", "-i"]).success().screen(), @r"
    default t1 · sh · 20x2 · cursor hidden · frame N
    1│just text
    …│ (row 2 blank)
    refs: none
    ");
}
