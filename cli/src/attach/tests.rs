use super::{Keys, Painter, Pressed};
use crate::protocol::Mirror;

fn calls(pressed: Vec<Pressed>) -> Vec<String> {
    pressed
        .into_iter()
        .map(|pressed| match pressed {
            Pressed::Human(input) => input.call().join(" "),
            Pressed::Detach => "detach".to_string(),
        })
        .collect()
}

fn decoded(bytes: &[u8]) -> Vec<String> {
    calls(Keys::default().feed(bytes))
}

fn table(inputs: &[&[u8]]) -> String {
    inputs
        .iter()
        .map(|bytes| format!("{:?} -> {}", String::from_utf8_lossy(bytes), decoded(bytes).join(" | ")))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn typed_text_runs_together_until_a_key_and_control_bytes_are_their_ctrl_keys() {
    insta::assert_snapshot!(table(&[b"ab", "é中".as_bytes(), b"ab\rcd", b"\t", b"\x7f", b"\x00", b"\x01", b"\x1a", b"\x1c", b"\x1d", b"\x1e", b"\x1f", b"\x08"]), @r##"
        "ab" -> type ab
        "é中" -> type é中
        "ab\rcd" -> type ab | press Enter | type cd
        "\t" -> press Tab
        "\u{7f}" -> press Backspace
        "\0" -> press Ctrl+Space
        "\u{1}" -> press Ctrl+a
        "\u{1a}" -> press Ctrl+z
        "\u{1c}" -> press Ctrl+\
        "\u{1d}" -> press Ctrl+]
        "\u{1e}" -> press Ctrl+^
        "\u{1f}" -> press Ctrl+_
        "\u{8}" -> press Ctrl+h
    "##);
}

#[test]
fn escape_sequences_are_the_arrows_function_and_editing_keys_with_their_modifiers() {
    insta::assert_snapshot!(table(&[b"\x1b[A", b"\x1b[1;5B", b"\x1b[1;2C", b"\x1b[1;3D", b"\x1b[1;8H", b"\x1b[F", b"\x1b[Z", b"\x1b[1;5Z", b"\x1bOP", b"\x1bOQ", b"\x1bOR", b"\x1bOS", b"\x1bOx", b"\x1b[x", b"\x1b[99~"]), @r##"
        "\u{1b}[A" -> press Up
        "\u{1b}[1;5B" -> press Ctrl+Down
        "\u{1b}[1;2C" -> press Shift+Right
        "\u{1b}[1;3D" -> press Alt+Left
        "\u{1b}[1;8H" -> press Ctrl+Alt+Shift+Home
        "\u{1b}[F" -> press End
        "\u{1b}[Z" -> press Shift+Tab
        "\u{1b}[1;5Z" -> press Ctrl+Shift+Tab
        "\u{1b}OP" -> press F1
        "\u{1b}OQ" -> press F2
        "\u{1b}OR" -> press F3
        "\u{1b}OS" -> press F4
        "\u{1b}Ox" -> 
        "\u{1b}[x" -> 
        "\u{1b}[99~" ->
    "##);
}

#[test]
fn tilde_numbers_name_the_editing_and_function_keys() {
    let tildes: Vec<Vec<u8>> = [1, 2, 3, 4, 5, 6, 7, 8, 11, 15, 17, 21, 23, 24, 9, 16, 22, 25]
        .iter()
        .map(|n| format!("\x1b[{n};2~").into_bytes())
        .collect();
    insta::assert_snapshot!(table(&tildes.iter().map(Vec::as_slice).collect::<Vec<_>>()), @r##"
        "\u{1b}[1;2~" -> press Shift+Home
        "\u{1b}[2;2~" -> press Shift+Insert
        "\u{1b}[3;2~" -> press Shift+Delete
        "\u{1b}[4;2~" -> press Shift+End
        "\u{1b}[5;2~" -> press Shift+PageUp
        "\u{1b}[6;2~" -> press Shift+PageDown
        "\u{1b}[7;2~" -> press Shift+Home
        "\u{1b}[8;2~" -> press Shift+End
        "\u{1b}[11;2~" -> press Shift+F1
        "\u{1b}[15;2~" -> press Shift+F5
        "\u{1b}[17;2~" -> press Shift+F6
        "\u{1b}[21;2~" -> press Shift+F10
        "\u{1b}[23;2~" -> press Shift+F11
        "\u{1b}[24;2~" -> press Shift+F12
        "\u{1b}[9;2~" -> 
        "\u{1b}[16;2~" -> 
        "\u{1b}[22;2~" -> 
        "\u{1b}[25;2~" ->
    "##);
}

#[test]
fn an_escape_before_a_key_is_alt_and_an_escape_alone_or_doubled_is_esc() {
    insta::assert_snapshot!(table(&[b"\x1ba", b"\x1b\x01", b"\x1b\x1b[A", b"\x1b", b"\x1b\x1b", b"\x1b\x10\x11", "\x1bé".as_bytes()]), @r##"
        "\u{1b}a" -> press Alt+a
        "\u{1b}\u{1}" -> press Ctrl+Alt+a
        "\u{1b}\u{1b}[A" -> press Esc | press Up
        "\u{1b}" -> press Esc
        "\u{1b}\u{1b}" -> press Esc | press Esc
        "\u{1b}\u{10}\u{11}" -> press Esc | detach
        "\u{1b}é" -> press Alt+é
    "##);
}

#[test]
fn ctrl_p_ctrl_q_detaches_and_ends_what_was_fed() {
    assert_eq!(decoded(b"ab\x10\x11cd"), ["type ab", "detach"]);
    assert_eq!(decoded(b"\x10x"), ["press Ctrl+p", "type x"]);
}

#[test]
fn a_sequence_split_across_reads_waits_for_its_end() {
    let mut keys = Keys::default();
    let fed: Vec<Vec<String>> =
        [&b"x\x1bO"[..], b"P\xc3", b"\xa9\x10", b"\x11"].into_iter().map(|bytes| calls(keys.feed(bytes))).collect();
    assert_eq!(
        fed,
        [
            vec!["type x".to_string()],
            vec!["press F1".to_string()],
            vec!["type é".to_string()],
            vec!["detach".to_string()]
        ]
    );
    assert_eq!(decoded(b"x\x1b"), ["type x", "press Esc"]);
    let mut csi = Keys::default();
    assert_eq!((calls(csi.feed(b"\x1b[1;")), calls(csi.feed(b"5A"))), (vec![], vec!["press Ctrl+Up".to_string()]));
}

#[test]
fn a_bracketed_paste_is_one_paste_even_across_reads() {
    assert_eq!(decoded(b"a\x1b[200~x\ry\x1b[201~b"), ["type a", "paste x\ry", "type b"]);
    let mut keys = Keys::default();
    let fed: Vec<Vec<String>> =
        [&b"\x1b[200~one "[..], b"two", b"\x1b[201~"].into_iter().map(|bytes| calls(keys.feed(bytes))).collect();
    assert_eq!(fed, [vec![], vec![], vec!["paste one two".to_string()]]);
}

#[test]
fn bytes_that_are_not_text_are_skipped() {
    assert_eq!(decoded(b"a\xffb\x80c"), ["type abc"]);
    assert_eq!(decoded(b"\xc3("), ["type ("]);
}

fn mirror(lines: &[&str]) -> Mirror {
    Mirror {
        program: "vim".to_string(),
        frame: 1,
        cols: 10,
        rows: u16::try_from(lines.len()).unwrap(),
        lines: lines.iter().map(ToString::to_string).collect(),
        cursor: [1, 1],
        cursor_visible: true,
        exited: false,
        exit_code: None,
    }
}

fn painter(read_only: bool) -> Painter {
    Painter { place: "work t1".to_string(), read_only, shown: Vec::new(), hint: String::new(), fitted: None }
}

#[test]
fn the_hint_names_the_tab_where_keys_go_or_how_the_program_ended() {
    let screen = mirror(&["a"]);
    let ended = |exit_code| Mirror { exited: true, exit_code, ..mirror(&["a"]) };
    assert_eq!(
        [
            painter(false).hint_for(&screen),
            painter(true).hint_for(&screen),
            painter(false).hint_for(&ended(Some(2))),
            painter(true).hint_for(&ended(None))
        ],
        [
            "work t1 · keys go to vim · ctrl-p ctrl-q detaches",
            "work t1 · read-only · ctrl-p ctrl-q detaches",
            "vim exited with code 2 · ctrl-p ctrl-q detaches",
            "vim exited · ctrl-p ctrl-q detaches"
        ]
    );
}

#[test]
fn a_repaint_redraws_only_the_rows_that_changed_and_a_new_fit_clears_the_screen() {
    let mut painting = painter(false);
    assert_eq!(
        (painting.refit((80, 24, 10, 2)), painting.refit((80, 24, 10, 2))),
        ("\x1b[0m\x1b[2J".to_string(), String::new())
    );
    let mut paint = String::new();
    painting.paint_rows(&mut paint, &mirror(&["one", "two"]), 24);
    painting.paint_rows(&mut paint, &mirror(&["one", "2"]), 1);
    painting.paint_rows(&mut paint, &mirror(&["one", "2"]), 24);
    assert_eq!(paint, "\x1b[1;1H\x1b[0m\x1b[2Kone\x1b[2;1H\x1b[0m\x1b[2Ktwo\x1b[2;1H\x1b[0m\x1b[2K2");
    let mut hinted = String::new();
    painting.paint_hint(&mut hinted, &mirror(&["one", "2"]), 12);
    painting.paint_hint(&mut hinted, &mirror(&["one", "2"]), 12);
    assert_eq!(hinted, "\x1b[3;1H\x1b[0m\x1b[2K\x1b[2mwork t1 · ke\x1b[0m");
    assert_eq!(painting.refit((80, 25, 10, 2)), "\x1b[0m\x1b[2J");
    assert!(painting.shown.is_empty() && painting.hint.is_empty());
}
