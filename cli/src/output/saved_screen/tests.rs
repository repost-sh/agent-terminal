use super::saved_rows;

#[test]
fn a_saved_screen_is_read_from_snapshot_json_or_from_numbered_text_rows() {
    let rows = saved_rows;
    let owned = |rows: &[&str]| Some(rows.iter().map(ToString::to_string).collect::<Vec<_>>());
    assert_eq!(rows(r#"{"first_row":3,"lines":["c","d"]}"#), owned(&["", "", "c", "d"]));
    assert_eq!(rows(r#"{"lines":["a"]}"#), owned(&["a"]));
    assert_eq!(
        [rows(r#"{"lines":[1]}"#), rows(r#"{"first_row":"x"}"#), rows(r#"{"first_row":-1,"lines":[]}"#)],
        [None, None, Some(Vec::new())]
    );
    assert_eq!(
        rows("work t1 · sh\n 1│one\n 3│three │ bar\n…│ (rows blank)\n 0│zero"),
        owned(&["one", "", "three │ bar"])
    );
    assert_eq!(rows("no rows here"), None);
}
