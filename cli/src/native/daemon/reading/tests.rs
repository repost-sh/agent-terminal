use serde_json::{Value, json};

use crate::native::daemon::tests::{Running, on, open, text};

const PAGE: &str = "printf 'one\\ntwo\\nthree\\nfour\\nfive\\n\\033[1mbold\\033[0m [ OK ]\\nready'; while read line; do printf '\\ngot %s' \"$line\"; done";

fn paged() -> Running {
    let running = Running::new();
    running.ok(&open(PAGE, &text("ready")));
    running
}

fn without_frame(mut reply: Value) -> Value {
    if let Some(fields) = reply.as_object_mut() {
        fields.remove("frame");
    }
    reply
}

fn snapshot(running: &Running, options: &Value) -> Value {
    without_frame(running.ok(&on("snapshot", &json!({ "options": options }))))
}

fn options(rows: Option<(u16, u16)>, scrollback: &Value, ansi: bool, refs: bool) -> Value {
    let rows = rows.map(|(first, last)| json!({ "first": first, "last": last }));
    json!({ "rows": rows, "scrollback": scrollback, "ansi": ansi, "refs": refs })
}

#[test]
fn a_snapshot_shows_the_screen_with_its_cursor_size_and_state() {
    let running = paged();
    insta::assert_snapshot!(snapshot(&running, &options(None, &json!("none"), false, false)), @r#"{"alt_screen":false,"cols":30,"cursor":[4,6],"cursor_visible":true,"exit_code":null,"exited":false,"first_row":1,"lines":["four","five","bold [ OK ]","ready"],"mouse":null,"program":"sh","rows":4,"session":"s","tab":"t1"}"#);
}

#[test]
fn a_snapshot_can_show_some_rows_the_scrollback_styles_and_refs() {
    let running = paged();
    let lines = |options: &Value| {
        let shown = snapshot(&running, options);
        let refs = shown["refs"].as_array().map(|refs| refs.iter().cloned().map(without_frame).collect::<Vec<_>>());
        format!(
            "from {} {} scrollback {} refs {}",
            shown["first_row"],
            shown["lines"],
            shown["scrollback"],
            json!(refs)
        )
    };
    let shown = [
        lines(&options(Some((2, 3)), &json!("none"), false, false)),
        lines(&options(Some((3, 99)), &json!("all"), false, false)),
        lines(&options(None, &json!({ "last": 2 }), false, false)),
        lines(&options(None, &json!({ "last": 99 }), false, false)),
        lines(&options(Some((3, 3)), &json!("none"), true, true)),
    ];
    insta::assert_snapshot!(shown.join("\n"), @r#"
        from 2 ["five","bold [ OK ]"] scrollback null refs null
        from 3 ["bold [ OK ]","ready"] scrollback ["one","two","three"] refs null
        from 1 ["four","five","bold [ OK ]","ready"] scrollback ["two","three"] refs null
        from 1 ["four","five","bold [ OK ]","ready"] scrollback ["one","two","three"] refs null
        from 3 ["\u001b[0;1mbold\u001b[0m [ OK ]"] scrollback null refs [{"col":8,"cols":[6,11],"kind":"button","label":"OK","ref":"@t1","row":3,"rows":[3,3]},{"col":6,"cols":[1,30],"kind":"input","label":"ready","ref":"@t2","row":4,"rows":[4,4]}]
    "#);
}

#[test]
fn rows_outside_the_screen_are_refused() {
    let running = paged();
    let refused = |first: u16, last: u16| {
        let request = on("snapshot", &json!({ "options": options(Some((first, last)), &json!("none"), false, false) }));
        running.refused(&request).1
    };
    assert_eq!(
        [refused(0, 1), refused(5, 6), refused(3, 2)],
        [
            "rows 0:1 are outside the 30x4 screen",
            "rows 5:6 are outside the 30x4 screen",
            "rows 3:2 are outside the 30x4 screen"
        ]
    );
}

fn fact(running: &Running, what: &Value) -> Value {
    running.ok(&on("get", &json!({ "what": what })))
}

#[test]
fn each_fact_reads_one_thing_about_the_tab() {
    let running = Running::new();
    running.ok(&open("printf '\\033]0;Title\\007\\033[?1h\\033[2;3Hready\\n\\n'; read x", &text("ready")));
    let facts = ["text", "cursor", "size", "program", "exit-code", "title", "cwd", "modes"]
        .map(|what| fact(&running, &json!(what)));
    insta::assert_snapshot!(facts.map(|fact| fact.to_string()).join("\n"), @r#"
        {"text":"\n  ready"}
        {"cursor":{"col":1,"row":4,"visible":true}}
        {"size":{"cols":30,"rows":4}}
        {"program":"sh"}
        {"exit_code":{"code":null,"exited":false}}
        {"title":"Title"}
        {"cwd":"/"}
        {"modes":{"alt_screen":false,"app_cursor":true,"bracketed_paste":false,"focus_events":false,"keyboard":[],"mouse":null}}
    "#);
    assert!(fact(&running, &json!("pid"))["pid"].as_u64().is_some_and(|pid| pid > 1));
    assert!(fact(&running, &json!("frame"))["frame"].as_u64().is_some_and(|frame| frame > 0));
    let cell = |row: u16, col: u16| on("get", &json!({ "what": { "cell": { "row": row, "col": col } } }));
    assert_eq!(running.ok(&cell(2, 3))["cell"]["char"], "r");
    assert_eq!(running.refused(&cell(9, 1)).1, "cell 9,1 is outside the 30x4 screen");
}

#[test]
fn find_lists_each_match_on_the_screen_or_with_all_in_the_scrollback_too() {
    let running = paged();
    let find = |text: &str, regex: bool, all: bool| on("find", &json!({ "text": text, "regex": regex, "all": all }));
    let found = [find("o", false, false), find("t.o", true, true), find("f", false, true)]
        .map(|request| running.ok(&request)["matches"].to_string());
    insta::assert_snapshot!(found.join("\n"), @r#"
        [{"col":2,"row":1,"text":"o"},{"col":2,"row":3,"text":"o"}]
        [{"col":1,"row":-2,"text":"two"}]
        [{"col":1,"row":1,"text":"f"},{"col":1,"row":2,"text":"f"}]
    "#);
    assert_eq!(
        [running.refused(&find("two", false, false)).1, running.refused(&find("zz", false, true)).1],
        ["\"two\" is not on the screen", "\"zz\" is not on the screen or in its scrollback"]
    );
    assert!(running.refused(&find("(", true, false)).1.starts_with("invalid regex \"(\": regex parse error:"));
}

fn typed(line: &str, until: &str) -> Value {
    on(
        "input",
        &json!({ "input": { "kind": "type", "text": format!("{line}\n"), "delay_ms": 0 }, "wait": text(until), "timeout_ms": 10_000 }),
    )
}

#[test]
fn output_since_a_mark_reads_what_the_program_printed_after_it() {
    let running = paged();
    let marked = running.ok(&on("mark", &json!({ "name": "before" })));
    assert_eq!((marked["mark"].as_str(), marked["frame"].is_u64()), (Some("before"), true));
    running.ok(&typed("x", "got x"));
    let output = |since: Option<&str>, raw: bool| {
        running.ok(&on("output", &json!({ "since": since, "raw": raw })))["output"].clone()
    };
    assert_eq!(
        [output(Some("before"), false), output(Some("before"), true)],
        [json!("x\n\ngot x"), json!("x\r\n\r\ngot x")]
    );
    assert_eq!(
        running.refused(&on("output", &json!({ "since": "nope", "raw": false }))).1,
        "no mark named \"nope\"; name a point with `agent-terminal mark nope`"
    );
}

#[test]
fn a_snapshot_diff_compares_with_the_last_snapshot_or_a_baseline() {
    let running = paged();
    let diff = |baseline: &Value| running.ok(&on("diff-snapshot", &json!({ "baseline": baseline })));
    running.ok(&typed("x", "got x"));
    let since_snapshot = diff(&Value::Null);
    assert!(since_snapshot["since"]["frame"].is_u64(), "{since_snapshot}");
    let baseline = diff(&json!({ "name": "before.txt", "lines": ["five", "bold [ OK ]"] }));
    insta::assert_snapshot!(format!("{}\n{}\n{}", since_snapshot["changes"], diff(&Value::Null)["changes"], without_frame(baseline)), @r#"
        [{"after":"bold [ OK ]","before":"four","row":1},{"after":"readyx","before":"five","row":2},{"after":"","before":"bold [ OK ]","row":3},{"after":"got x","before":"ready","row":4}]
        []
        {"changes":[{"after":"bold [ OK ]","before":"five","row":1},{"after":"readyx","before":"bold [ OK ]","row":2},{"after":"got x","before":"","row":4}],"since":{"baseline":"before.txt"}}
    "#);
}
