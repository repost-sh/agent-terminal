use crate::output::tests::replied;

#[test]
fn deliveries_waits_and_checks_read_as_what_happened() {
    insta::assert_snapshot!(replied(&[
        ("Delivered", r#"{"bytes":3,"input":"text","keys":3,"session":"rec","tab":"app"}"#),
        ("Delivered", r#"{"bytes":1,"input":"keys","keys":1,"session":"rec","tab":"app"}"#),
        ("Waited", r#"{"frame":5,"session":"rec","tab":"app","until":"\"got\" to appear","waited_ms":0}"#),
        ("Checked", r#"{"fact":"\"got\" is on screen","session":"rec","tab":"app"}"#),
    ]), @r##"
        typed 3 keys into rec app

        ~
        pressed 1 key in rec app

        ~
        waited 0 ms for "got" to appear in rec app

        ~
        "got" is on screen in rec app
    "##);
}

#[test]
fn pastes_bytes_signals_and_scrolls_say_what_was_sent() {
    insta::assert_snapshot!(replied(&[
        ("Delivered", r#"{"bracketed":false,"bytes":6,"input":"paste","session":"rec","tab":"app"}"#),
        ("Delivered", r#"{"bracketed":true,"bytes":6,"input":"paste","session":"rec","tab":"app"}"#),
        ("Delivered", r#"{"bytes":1,"input":"raw","session":"rec","tab":"app"}"#),
        ("Delivered", r#"{"input":"signal","process_group":4321,"session":"rec","signal":"INT","tab":"app"}"#),
        ("Delivered", r#"{"by":"keys","bytes":8,"count":2,"direction":"up","input":"scroll","session":"rec","tab":"app"}"#),
        ("Delivered", r#"{"by":"wheel","bytes":8,"count":1,"direction":"down","input":"scroll","session":"rec","tab":"app"}"#),
    ]), @r##"
        pasted 6 bytes into rec app (plain, the app has not turned on bracketed paste)

        ~
        pasted 6 bytes into rec app (bracketed paste)

        ~
        sent 1 raw byte to rec app

        ~
        sent INT to rec app (process group 4321)

        ~
        scrolled up 2 in rec app with PageUp

        ~
        scrolled down 1 in rec app with the mouse wheel
    "##);
}

#[test]
fn clicks_and_mouse_events_say_where_and_with_which_button() {
    insta::assert_snapshot!(replied(&[
        ("Delivered", r#"{"input":"click","target":"@t3","row":2,"col":4,"button":"left","double":false,"bytes":12,"session":"rec","tab":"app"}"#),
        ("Delivered", r#"{"input":"click","target":"4,3","row":4,"col":3,"button":"right","double":true,"bytes":24,"session":"rec","tab":"app"}"#),
        ("Delivered", r#"{"input":"click","target":"OK","row":1,"col":9,"button":"middle","double":false,"bytes":12,"session":"rec","tab":"app"}"#),
        ("Delivered", r#"{"input":"mouse","event":"move","row":2,"col":3,"bytes":6,"session":"rec","tab":"app"}"#),
        ("Delivered", r#"{"input":"mouse","event":"down","row":2,"col":3,"button":"left","bytes":6,"session":"rec","tab":"app"}"#),
        ("Delivered", r#"{"input":"mouse","event":"up","row":2,"col":3,"button":"right","bytes":6,"session":"rec","tab":"app"}"#),
        ("Delivered", r#"{"input":"mouse","event":"wheel","row":2,"col":3,"direction":"up","bytes":6,"session":"rec","tab":"app"}"#),
        ("Delivered", r#"{"input":"click","target":"4,3","row":4,"col":3,"button":"right","double":false,"mods":{"ctrl":true},"bytes":12,"session":"rec","tab":"app"}"#),
        ("Delivered", r#"{"input":"mouse","event":"move","row":2,"col":3,"mods":{"shift":true,"alt":true},"bytes":0,"session":"rec","tab":"app"}"#),
        ("Delivered", r#"{"input":"focus","focused":true,"bytes":3,"session":"rec","tab":"app"}"#),
        ("Delivered", r#"{"input":"focus","focused":false,"bytes":0,"session":"rec","tab":"app"}"#),
    ]), @r##"
        clicked @t3 at 2,4 in rec app

        ~
        clicked 4,3 in rec app (right button, double click)

        ~
        clicked "OK" at 1,9 in rec app (middle button)

        ~
        mouse move to 2,3 in rec app

        ~
        mouse down at 2,3 in rec app

        ~
        mouse up at 2,3 in rec app (right button)

        ~
        mouse wheel up at 2,3 in rec app

        ~
        clicked 4,3 in rec app (right button, Ctrl held)

        ~
        mouse move to 2,3 in rec app (Alt+Shift held, not sent: the app does not track this event)

        ~
        reported focus in to rec app

        ~
        reported focus out to rec app (not sent: the app does not ask for focus reports)
    "##);
}

#[test]
fn a_delivery_that_asked_for_the_screen_prints_it_after_what_it_sent() {
    insta::assert_snapshot!(replied(&[
        ("Delivered", r#"{"bracketed":false,"bytes":6,"input":"paste","session":"rec","tab":"app","snapshot":{"alt_screen":false,"cols":30,"cursor":[3,1],"cursor_visible":true,"exit_code":null,"exited":false,"first_row":1,"frame":1,"lines":["hello [ OK ]","q: quit  h: help","","","",""],"mouse":null,"program":"sh","refs":[{"col":9,"cols":[7,12],"frame":1,"kind":"button","label":"OK","ref":"@t1","row":1,"rows":[1,1]},{"col":1,"cols":[1,7],"frame":1,"kind":"hint","label":"q: quit","ref":"@t2","row":2,"rows":[2,2]},{"col":10,"cols":[10,16],"frame":1,"kind":"hint","label":"h: help","ref":"@t3","row":2,"rows":[2,2]},{"col":1,"cols":[1,30],"frame":1,"kind":"input","label":"","ref":"@t4","row":3,"rows":[3,3]}],"rows":6,"session":"rec","tab":"app"}}"#),
    ]), @r##"
        pasted 6 bytes into rec app (plain, the app has not turned on bracketed paste)
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
