use crate::protocol::{Reply, Squeeze};

use super::{reply_data, reply_text};

pub(super) fn replied(replies: &[(&str, &str)]) -> String {
    replies
        .iter()
        .map(|(reply, data)| {
            let data: serde_json::Value = serde_json::from_str(data).unwrap();
            let reply: Reply = serde_json::from_value(serde_json::json!({ "reply": reply, "data": data })).unwrap();
            reply_text(&reply, &Squeeze::Off)
        })
        .collect::<Vec<_>>()
        .join("\n~\n")
}

#[test]
fn a_screen_reads_as_its_header_numbered_rows_and_refs() {
    insta::assert_snapshot!(replied(&[
        ("Snapshot", r#"{"alt_screen":false,"cols":30,"cursor":[3,1],"cursor_visible":true,"exit_code":null,"exited":false,"first_row":1,"frame":1,"lines":["hello [ OK ]","q: quit  h: help","","","",""],"mouse":null,"program":"sh","refs":[{"col":9,"cols":[7,12],"frame":1,"kind":"button","label":"OK","ref":"@t1","row":1,"rows":[1,1]},{"col":1,"cols":[1,7],"frame":1,"kind":"hint","label":"q: quit","ref":"@t2","row":2,"rows":[2,2]},{"col":10,"cols":[10,16],"frame":1,"kind":"hint","label":"h: help","ref":"@t3","row":2,"rows":[2,2]},{"col":1,"cols":[1,30],"frame":1,"kind":"input","label":"","ref":"@t4","row":3,"rows":[3,3]}],"rows":6,"session":"rec","tab":"app"}"#),
    ]), @r##"
        rec app · sh · 30x6 · cursor 3,1 · frame 1
        1│hello [ OK ]
        2│q: quit  h: help
        …│ (rows 3-6 blank)
        refs:
          @t1 [button] "OK"      row 1 col 9
          @t2 [hint]   "q: quit" row 2 col 1
          @t3 [hint]   "h: help" row 2 col 10
          @t4 [input]  ""        row 3 col 1
    "##);
}

#[test]
fn a_part_of_the_screen_starts_at_its_first_row() {
    insta::assert_snapshot!(replied(&[
        ("Snapshot", r#"{"alt_screen":false,"cols":30,"cursor":[3,1],"cursor_visible":true,"exit_code":null,"exited":false,"first_row":1,"frame":1,"lines":["hello [ OK ]","q: quit  h: help"],"mouse":null,"program":"sh","rows":6,"scrollback":[],"session":"rec","tab":"app"}"#),
    ]), @r##"
        rec app · sh · 30x6 · cursor 3,1 · frame 1
        1│hello [ OK ]
        2│q: quit  h: help
    "##);
}

#[test]
fn an_ended_program_on_the_alternate_screen_says_so_in_the_header() {
    insta::assert_snapshot!(replied(&[
        ("Snapshot", r#"{"alt_screen":true,"cols":30,"cursor":[3,1],"cursor_visible":false,"exit_code":2,"exited":true,"first_row":1,"frame":1,"lines":["done","","","","",""],"mouse":"sgr","program":"sh","rows":6,"session":"rec","tab":"app"}"#),
    ]), @r##"
        rec app · sh · 30x6 · alt-screen · mouse:sgr · cursor hidden · frame 1 · exited 2
        1│done
        …│ (rows 2-6 blank)
    "##);
}

#[test]
fn scrollback_rows_come_above_the_screen_with_negative_numbers() {
    insta::assert_snapshot!(replied(&[
        ("Snapshot", r#"{"alt_screen":false,"cols":30,"cursor":[3,1],"cursor_visible":true,"exit_code":null,"exited":false,"first_row":3,"frame":1,"lines":["row three","row four"],"mouse":null,"program":"sh","rows":6,"scrollback":["earlier one","earlier two"],"session":"rec","tab":"app"}"#),
    ]), @r##"
        rec app · sh · 30x6 · cursor 3,1 · frame 1
        -2│earlier one
        -1│earlier two
         3│row three
         4│row four
    "##);
}

#[test]
fn squeezing_shapes_the_text_of_a_screen_and_of_the_screen_after_input_and_leaves_the_json_as_it_is() {
    let shown = r#"{"alt_screen":false,"cols":30,"cursor":[1,1],"cursor_visible":true,"exit_code":null,"exited":false,"first_row":1,"frame":1,"lines":["a","","","","b"],"mouse":null,"program":"sh","rows":5,"session":"rec","tab":"app"}"#;
    let delivered =
        format!(r#"{{"input":"paste","bytes":1,"bracketed":false,"session":"rec","tab":"app","snapshot":{shown}}}"#);
    let replies: [Reply; 2] = [("Snapshot", shown.to_string()), ("Delivered", delivered)]
        .map(|(reply, data)| serde_json::from_str(&format!(r#"{{"reply":"{reply}","data":{data}}}"#)).unwrap());
    for reply in &replies {
        let (squeezed, kept) = (reply_text(reply, &Squeeze::On), reply_text(reply, &Squeeze::Off));
        assert!(squeezed.contains("…│ (rows 2-4 blank)") && !kept.contains("blank"), "{squeezed}\n{kept}");
        assert!(!reply_data(reply).unwrap().to_string().contains("blank"));
    }
}

#[test]
fn a_diff_lists_each_changed_row_or_says_nothing_changed() {
    insta::assert_snapshot!(replied(&[
        ("Diff", r#"{"changes":[{"after":"abc","before":"","row":3},{"after":"got abc","before":"","row":4}],"frame":5,"since":{"frame":1}}"#),
        ("Diff", r#"{"changes":[],"frame":5,"since":{"frame":5}}"#),
    ]), @r##"
        frame 1 → frame 5 · 2 rows changed
        -3│
        +3│abc
        -4│
        +4│got abc

        ~
        frame 5 → frame 5 · no rows changed
    "##);
}

#[test]
fn the_screens_facts_read_as_one_line_each() {
    insta::assert_snapshot!(replied(&[
        ("Fact", r#"{"text":"hello [ OK ]\nq: quit  h: help"}"#),
        ("Fact", r#"{"cursor":{"col":1,"row":3,"visible":true}}"#),
        ("Fact", r#"{"size":{"cols":30,"rows":6}}"#),
        ("Fact", r#"{"frame":1}"#),
        ("Fact", r#"{"title":null}"#),
        ("Fact", r#"{"cwd":"/home/me/work"}"#),
        ("Fact", r#"{"cursor":{"col":1,"row":3,"visible":false}}"#),
        ("Fact", r#"{"title":"vim"}"#),
    ]), @r##"
        hello [ OK ]
        q: quit  h: help

        ~
        3,1

        ~
        30x6

        ~
        1

        ~

        ~
        /home/me/work

        ~
        hidden

        ~
        vim
    "##);
}

#[test]
fn the_programs_facts_read_as_one_line_each() {
    insta::assert_snapshot!(replied(&[
        ("Fact", r#"{"program":"sh"}"#),
        ("Fact", r#"{"exit_code":{"code":null,"exited":false}}"#),
        ("Fact", r#"{"pid":4242}"#),
        ("Fact", r#"{"exit_code":{"code":2,"exited":true}}"#),
        ("Fact", r#"{"exit_code":{"code":null,"exited":true}}"#),
        ("Fact", r#"{"pid":null}"#),
    ]), @r##"
        sh

        ~
        running

        ~
        4242

        ~
        2

        ~
        exited, exit code unknown

        ~
        unknown
    "##);
}

#[test]
fn modes_and_cells_read_as_their_settings() {
    insta::assert_snapshot!(replied(&[
        ("Fact", r#"{"modes":{"alt_screen":false,"app_cursor":false,"bracketed_paste":false,"focus_events":false,"keyboard":[],"mouse":null}}"#),
        ("Fact", r#"{"modes":{"alt_screen":true,"app_cursor":true,"bracketed_paste":true,"focus_events":true,"keyboard":["disambiguate","report-events"],"mouse":{"tracking":"drag","encoding":"sgr"}}}"#),
        ("Fact", r#"{"cell":{"bg":"default","bold":true,"char":"h","col":1,"fg":"default","inverse":false,"row":1,"underline":false}}"#),
        ("Fact", r##"{"cell":{"bg":"#010203","bold":false,"char":" ","col":2,"fg":"red","inverse":true,"row":4,"underline":true}}"##),
    ]), @r##"
        alt-screen: off
        mouse: off
        bracketed-paste: off
        app-cursor: off
        focus-events: off
        keyboard: legacy

        ~
        alt-screen: on
        mouse: drag, sgr
        bracketed-paste: on
        app-cursor: on
        focus-events: on
        keyboard: disambiguate, report-events

        ~
        "h" fg=default bg=default bold

        ~
        " " fg=red bg=#010203 inverse underline
    "##);
}

#[test]
fn matches_read_as_their_places_and_marks_as_their_frame() {
    insta::assert_snapshot!(replied(&[
        ("Matches", r#"{"matches":[{"col":9,"row":1,"text":"OK"}]}"#),
        ("Matches", r#"{"matches":[]}"#),
        ("Matches", r#"{"matches":[{"col":2,"row":-3,"text":"a"},{"col":5,"row":1,"text":"bc"}]}"#),
        ("Marked", r#"{"frame":1,"mark":"here"}"#),
    ]), @r##"
        1,9 OK

        ~

        ~
        -3,2 a
        1,5 bc

        ~
        marked here at frame 1
    "##);
}

#[test]
fn printed_output_is_given_as_it_was_printed() {
    insta::assert_snapshot!(replied(&[
        ("Printed", r#"{"output":"abc\ngot abc\n"}"#),
        ("Printed", r#"{"output":""}"#),
        ("Printed", r#"{"output":"no newline"}"#),
    ]), @r##"
        abc
        got abc

        ~

        ~
        no newline
    "##);
}

#[test]
fn sessions_and_tabs_read_as_lists() {
    insta::assert_snapshot!(replied(&[
        ("Sessions", r#"{"sessions":[{"current":"app","session":"rec","tabs":[{"cols":30,"command":"sh -c \"printf \\\"\\\\033[1mhello\\\\033[0m [ OK ]\\\\nq: quit  h: help\\\\n\\\"; read x; printf \\\"got \"'$x'\"\\\\n\\\"; sleep 30\"","control":"agent","cwd":"/home/me/work","exit_code":null,"exited":false,"program":"sh","rows":6,"tab":"app"}]}]}"#),
        ("Sessions", r#"{"sessions":[]}"#),
        ("Sessions", r#"{"sessions":[{"current":null,"session":"idle","tabs":[]}]}"#),
        ("Tabs", r#"{"current":"app","session":"rec","tabs":[{"cols":30,"command":"sh -c \"printf \\\"\\\\033[1mhello\\\\033[0m [ OK ]\\\\nq: quit  h: help\\\\n\\\"; read x; printf \\\"got \"'$x'\"\\\\n\\\"; sleep 30\"","control":"agent","cwd":"/home/me/work","exit_code":null,"exited":false,"program":"sh","rows":6,"tab":"app"}]}"#),
        ("Tabs", r#"{"current":null,"session":"idle","tabs":[]}"#),
        ("Created", r#"{"session":"repost-forward"}"#),
    ]), @r##"
        rec app · sh · 30x6 · running · current

        ~
        no sessions are open

        ~
        idle · no tabs

        ~
        rec app · sh · 30x6 · running · current

        ~
        no tabs are open in session idle

        ~
        created session repost-forward
    "##);
}

#[test]
fn screenshots_exports_and_closes_say_where_and_what() {
    insta::assert_snapshot!(replied(&[
        ("Screenshot", r#"{"format":"png","frame":5,"height":140,"path":"/home/me/work/shots/a.png","session":"rec","tab":"app","width":290}"#),
        ("Exported", r#"{"bytes":7098,"files":3,"path":"/home/me/work/t.zip","session":"rec"}"#),
        ("Exported", r#"{"bytes":10,"files":1,"path":"/t.zip","session":"rec","missing":["/a.png","/b.png"]}"#),
        ("Closed", r#"{"closed":[{"session":"rec","tab":"app"}]}"#),
        ("Closed", r#"{"closed":[]}"#),
        ("Closed", r#"{"closed":[{"session":"a","tab":"app"},{"session":"b","tab":"api"}]}"#),
        ("Closed", r#"{"closed":[{"session":"a","tab":"app"}],"ended":["a","b"]}"#),
    ]), @r##"
        saved /home/me/work/shots/a.png (rec app, frame 5)

        ~
        exported session rec to /home/me/work/t.zip (3 files, 7 KB)

        ~
        exported session rec to /t.zip (1 file, 1 KB)
        2 files the session saved are gone and are not in it: /a.png, /b.png

        ~
        closed rec app

        ~

        ~
        closed a app
        closed b api

        ~
        closed a app
        ended session a
        ended session b
    "##);
}

#[test]
fn a_shell_command_reads_as_its_output_and_exit_and_history_as_its_lines() {
    insta::assert_snapshot!(replied(&[
        ("Command", r#"{"command":"echo hi; false","cwd":"/home/me/work","duration_ms":1234,"ended_at_ms":1700000001234,"exit_code":1,"first_frame":4,"id":1,"last_frame":5,"output":"hi\n","session":"rec","started_at_ms":1700000000000,"tab":"app"}"#),
        ("Command", r#"{"check":true,"command":"echo done","cwd":"/home/me/work","duration_ms":5,"ended_at_ms":1700000001234,"exit_code":0,"first_frame":12,"id":3,"last_frame":13,"output":"done\n","session":"rec","started_at_ms":1700000000000,"tab":"app"}"#),
        ("Command", r#"{"command":"echo hi; false","cwd":"/home/me/work","duration_ms":61000,"ended_at_ms":null,"exit_code":null,"first_frame":4,"id":1,"last_frame":5,"output":"hi\n","session":"rec","started_at_ms":1700000000000,"tab":"app"}"#),
        ("History", r#"{"commands":[{"command":"echo hi; false","cwd":"/home/me/work","duration_ms":0,"ended_at_ms":1700000001234,"exit_code":1,"first_frame":4,"id":1,"last_frame":5,"started_at_ms":1700000000000},{"command":"echo hi; false","cwd":"/home/me/work","duration_ms":999,"ended_at_ms":1700000001234,"exit_code":1,"first_frame":8,"id":2,"last_frame":9,"started_at_ms":1700000000000},{"command":"echo done","cwd":"/home/me/work","duration_ms":1000,"ended_at_ms":1700000001234,"exit_code":0,"first_frame":12,"id":3,"last_frame":13,"started_at_ms":1700000000000},{"command":"echo done","cwd":"/home/me/work","duration_ms":125000,"ended_at_ms":1700000001234,"exit_code":0,"first_frame":16,"id":4,"last_frame":17,"started_at_ms":1700000000000}],"session":"rec","tab":"app","total":4}"#),
        ("History", r#"{"commands":[],"session":"rec","tab":"app","total":0}"#),
    ]), @r##"
        hi
        1 · exit 1 · 1.2 s · /home/me/work · echo hi; false

        ~
        done
        3 · exit 0 · 5 ms · /home/me/work · echo done

        ~
        hi
        1 · running · 1 min 01 s · /home/me/work · echo hi; false

        ~
        1 · exit 1 · 0 ms · /home/me/work · echo hi; false
        2 · exit 1 · 999 ms · /home/me/work · echo hi; false
        3 · exit 0 · 1.0 s · /home/me/work · echo done
        4 · exit 0 · 2 min 05 s · /home/me/work · echo done

        ~
        no commands have run in rec app
    "##);
}

#[test]
fn a_screenshot_diff_counts_the_pixels_that_differ_against_its_threshold() {
    insta::assert_snapshot!(replied(&[
        ("ScreenshotDiff", r#"{"baseline":"/home/me/work/shots/base.png","changed":491,"frame":4,"passed":false,"path":"/home/me/work/shots/d.png","pixels":16000,"session":"rec","tab":"app","threshold":0.0}"#),
        ("ScreenshotDiff", r#"{"baseline":"/home/me/work/shots/base.png","changed":491,"frame":4,"passed":true,"path":"/home/me/work/shots/d.png","pixels":16000,"session":"rec","tab":"app","threshold":0.5}"#),
        ("ScreenshotDiff", r#"{"baseline":"/home/me/work/shots/base.png","changed":0,"frame":4,"passed":true,"path":"/home/me/work/shots/d.png","pixels":16000,"session":"rec","tab":"app","threshold":0.0}"#),
    ]), @r##"
        491 of 16000 pixels (3.07%) differ from /home/me/work/shots/base.png, above the threshold 0; diff image /home/me/work/shots/d.png

        ~
        491 of 16000 pixels (3.07%) differ from /home/me/work/shots/base.png, within the threshold 0.5; diff image /home/me/work/shots/d.png

        ~
        0 of 16000 pixels (0.00%) differ from /home/me/work/shots/base.png, within the threshold 0; diff image /home/me/work/shots/d.png
    "##);
}

#[test]
fn the_dashboard_says_where_it_listens_and_whether_it_stopped() {
    insta::assert_snapshot!(replied(&[
        ("Dashboard", r#"{"port":4849,"url":"http://localhost:4849"}"#),
        ("DashboardStopped", r#"{"stopped":true}"#),
        ("DashboardStopped", r#"{"stopped":false}"#),
        ("KeptRunning", r#"{"terminals":2}"#),
        ("KeptRunning", r#"{"terminals":1}"#),
    ]), @r##"
        dashboard running at http://localhost:4849

        ~
        dashboard stopped

        ~
        no dashboard is running

        ~

        ~
    "##);
}

#[test]
fn a_batch_numbers_each_call_with_its_exit_and_what_it_printed() {
    insta::assert_snapshot!(replied(&[
        ("Batch", r#"[{"args":["get","size"],"response":{"Ok":{"reply":"Fact","data":{"size":{"cols":40,"rows":8}}}}},{"args":["wait","x"],"response":{"Err":{"code":3,"message":"timed out"}}},{"args":["press","a b"],"response":null}]"#),
    ]), @r##"
        [1] get size · exit 0
        40x8

        [2] wait x · exit 3
        error: timed out

        [3] press 'a b' · not run
    "##);
}

#[test]
fn a_batch_in_json_gives_each_calls_result_or_error_and_its_exit() {
    let outcomes: Vec<crate::protocol::BatchOutcome> = serde_json::from_str(
        r#"[{"args":["get","frame"],"response":{"Ok":{"reply":"Fact","data":{"frame":3}}}},{"args":["x"],"response":{"Err":{"code":1,"message":"no"}}},{"args":["y"],"response":null}]"#,
    )
    .unwrap();
    assert_eq!(
        super::batch::batch_json(&outcomes).unwrap().to_string(),
        r#"[{"args":["get","frame"],"exit_code":0,"result":{"frame":3}},{"args":["x"],"error":{"code":1,"message":"no"},"exit_code":1},{"args":["y"],"exit_code":null}]"#
    );
}
