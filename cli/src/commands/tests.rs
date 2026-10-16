use std::ffi::OsString;

use clap::CommandFactory;

use super::programs::NAME_THE_TAB;
use super::{Cli, argv, parsed, request, unnamed_tab};

fn own_parts(mut sent: serde_json::Value) -> String {
    if let Some(env) = sent["env"].as_array_mut() {
        env.retain(|pair| std::env::var(pair[0].as_str().unwrap()).ok().as_deref() != pair[1].as_str());
    }
    sent.to_string().replace(&*std::env::current_dir().unwrap().to_string_lossy(), "<here>")
}

fn sent(words: Vec<String>) -> String {
    let argv: Vec<OsString> = std::iter::once("agent-terminal".to_string()).chain(words).map(OsString::from).collect();
    let mut parser = Cli::command().mut_arg("session", |arg| arg.env(None::<&str>));
    match argv::parse(&mut parser, &argv).and_then(parsed) {
        Ok(cli) => request(cli).map_or_else(
            |refused| format!("refused: {refused}"),
            |request| own_parts(serde_json::to_value(&request).unwrap()),
        ),
        Err(e) if unnamed_tab(&e) => format!("refused: {NAME_THE_TAB}"),
        Err(e) => format!("usage: {:?}", e.kind()),
    }
}

fn requested(line: &str) -> String {
    let global = ["--session", "work", "--screenshot-dir", "/shots"].map(String::from);
    format!("{line} -> {}", sent(global.into_iter().chain(shlex::split(line).unwrap()).collect()))
}

fn table(lines: &[&str]) -> String {
    lines.iter().map(|line| requested(line)).collect::<Vec<_>>().join("\n")
}

fn typed_alone(line: &str) -> String {
    format!("{line} -> {}", sent(shlex::split(line).unwrap()))
}

#[test]
fn close_all_closes_the_session_typed_on_its_own_command_line_or_with_bypass_session_every_session() {
    let lines = [
        "close --all --session a",
        "close --all --bypass-session",
        "close --bypass-session --all --session a",
        "close --all",
    ];
    insta::assert_snapshot!(lines.map(typed_alone).join("\n"), @r#"
        close --all --session a -> {"command":"close","env":null,"scope":"session","session":"a"}
        close --all --bypass-session -> {"command":"close","env":null,"scope":"every-session"}
        close --bypass-session --all --session a -> refused: the argument '--bypass-session' cannot be used with '--session <NAME>': close --all --session <name> closes the tabs of that session, close --all --bypass-session closes every tab in every session
        close --all -> refused: close --all closes every tab of one session, so name that session on this command line: repeat it as `--session <name>` (AGENT_TERMINAL_SESSION alone is not enough); `close --all --bypass-session` closes every tab in every session
    "#);
}

#[test]
fn a_session_is_created_by_session_new_and_named_on_every_command_that_acts_on_one() {
    let lines = [
        "session new repost-forward",
        "session new Repost",
        "session new a-name-longer-than-24-chars",
        "session list",
        "session id",
        "close --all --bypass-session",
        "snapshot",
        "open --name top htop",
        "tab",
        "close",
        "--session Work snapshot",
        "--session 9lives snapshot",
    ];
    insta::assert_snapshot!(lines.map(typed_alone).join("\n"), @r#"
        session new repost-forward -> {"command":"new-session","env":null,"session":"repost-forward"}
        session new Repost -> usage: ValueValidation
        session new a-name-longer-than-24-chars -> usage: ValueValidation
        session list -> {"command":"sessions","env":null}
        session id -> usage: InvalidSubcommand
        close --all --bypass-session -> {"command":"close","env":null,"scope":"every-session"}
        snapshot -> refused: name this conversation's session after its task and create it first: `agent-terminal session new repost-forward`, then pass `--session repost-forward` on every call (or export AGENT_TERMINAL_SESSION)
        open --name top htop -> refused: name this conversation's session after its task and create it first: `agent-terminal session new repost-forward`, then pass `--session repost-forward` on every call (or export AGENT_TERMINAL_SESSION)
        tab -> refused: name this conversation's session after its task and create it first: `agent-terminal session new repost-forward`, then pass `--session repost-forward` on every call (or export AGENT_TERMINAL_SESSION)
        close -> refused: name this conversation's session after its task and create it first: `agent-terminal session new repost-forward`, then pass `--session repost-forward` on every call (or export AGENT_TERMINAL_SESSION)
        --session Work snapshot -> usage: ValueValidation
        --session 9lives snapshot -> {"command":"snapshot","env":null,"options":{"ansi":false,"refs":false,"rows":null,"scrollback":"none","squeeze_blank":false},"session":"9lives","tab":null}
    "#);
}

#[test]
fn opening_a_program_takes_its_options_before_or_after_it_and_keeps_its_own_flags() {
    insta::assert_snapshot!(table(&[
        "open --name notes vim notes.txt",
        "open --name top --size 80x24 --cwd /srv --env A=1 --env B=2 htop -d 5",
        "open htop --size 80x24 --wait ready --name top",
        "open --name top htop --wait-stable",
        "open --name top htop --wait-stable 50 -d 5",
        "open --name top --no-wait -- htop --size 1x1 --name x",
        "open --name ls ls -- --size",
        "open --name top htop --wait-stable=40",
        "shell --name sh --shell zsh --size 100x30 --wait $",
        "shell --shell bash --no-wait --name sh",
        "tab new --name make --cwd /tmp make",
        "tab new make all --env X=y --name make",
        "open --name top --size 9 htop",
    ]), @r##"
        open --name notes vim notes.txt -> {"args":["notes.txt"],"command":"open","cwd":"<here>","env":[],"program":"vim","session":"work","size":{"cols":120,"rows":40},"tab":"notes","timeout_ms":25000,"wait":{"quiet_ms":300,"until":"stable"}}
        open --name top --size 80x24 --cwd /srv --env A=1 --env B=2 htop -d 5 -> {"args":["-d","5"],"command":"open","cwd":"/srv","env":[["A","1"],["B","2"]],"program":"htop","session":"work","size":{"cols":80,"rows":24},"tab":"top","timeout_ms":25000,"wait":{"quiet_ms":300,"until":"stable"}}
        open htop --size 80x24 --wait ready --name top -> {"args":[],"command":"open","cwd":"<here>","env":[],"program":"htop","session":"work","size":{"cols":80,"rows":24},"tab":"top","timeout_ms":25000,"wait":{"regex":false,"rows":null,"text":"ready","until":"text"}}
        open --name top htop --wait-stable -> {"args":[],"command":"open","cwd":"<here>","env":[],"program":"htop","session":"work","size":{"cols":120,"rows":40},"tab":"top","timeout_ms":25000,"wait":{"quiet_ms":300,"until":"stable"}}
        open --name top htop --wait-stable 50 -d 5 -> {"args":["-d","5"],"command":"open","cwd":"<here>","env":[],"program":"htop","session":"work","size":{"cols":120,"rows":40},"tab":"top","timeout_ms":25000,"wait":{"quiet_ms":50,"until":"stable"}}
        open --name top --no-wait -- htop --size 1x1 --name x -> {"args":["--size","1x1","--name","x"],"command":"open","cwd":"<here>","env":[],"program":"htop","session":"work","size":{"cols":120,"rows":40},"tab":"top","timeout_ms":25000,"wait":null}
        open --name ls ls -- --size -> {"args":["--size"],"command":"open","cwd":"<here>","env":[],"program":"ls","session":"work","size":{"cols":120,"rows":40},"tab":"ls","timeout_ms":25000,"wait":{"quiet_ms":300,"until":"stable"}}
        open --name top htop --wait-stable=40 -> {"args":[],"command":"open","cwd":"<here>","env":[],"program":"htop","session":"work","size":{"cols":120,"rows":40},"tab":"top","timeout_ms":25000,"wait":{"quiet_ms":40,"until":"stable"}}
        shell --name sh --shell zsh --size 100x30 --wait $ -> {"command":"shell","cwd":"<here>","env":[],"program":"zsh","session":"work","shell":"zsh","size":{"cols":100,"rows":30},"tab":"sh","timeout_ms":25000,"wait":{"regex":false,"rows":null,"text":"$","until":"text"}}
        shell --shell bash --no-wait --name sh -> {"command":"shell","cwd":"<here>","env":[],"program":"bash","session":"work","shell":"bash","size":{"cols":120,"rows":40},"tab":"sh","timeout_ms":25000,"wait":null}
        tab new --name make --cwd /tmp make -> {"args":[],"command":"open","cwd":"/tmp","env":[],"program":"make","session":"work","size":{"cols":120,"rows":40},"tab":"make","timeout_ms":25000,"wait":{"quiet_ms":300,"until":"stable"}}
        tab new make all --env X=y --name make -> {"args":["all"],"command":"open","cwd":"<here>","env":[["X","y"]],"program":"make","session":"work","size":{"cols":120,"rows":40},"tab":"make","timeout_ms":25000,"wait":{"quiet_ms":300,"until":"stable"}}
        open --name top --size 9 htop -> usage: ValueValidation
    "##);
}

#[test]
fn every_way_of_opening_a_tab_needs_a_name_that_follows_the_rule() {
    insta::assert_snapshot!(table(&[
        "open htop",
        "tab new make",
        "shell --shell zsh",
        "open --name Api htop",
    ]), @r##"
        open htop -> refused: name the tab after the program or app it runs, e.g. `--name api`; the name is how later calls reach it (`--tab api`)
        tab new make -> refused: name the tab after the program or app it runs, e.g. `--name api`; the name is how later calls reach it (`--tab api`)
        shell --shell zsh -> refused: name the tab after the program or app it runs, e.g. `--name api`; the name is how later calls reach it (`--tab api`)
        open --name Api htop -> usage: ValueValidation
    "##);
}

#[test]
fn shell_history_restart_and_resize_name_their_command_and_its_wait() {
    insta::assert_snapshot!(table(&[
        "exec 'ls -la'",
        "exec --check true",
        "history",
        "history --last 3 --failed",
        "history show 4",
        "restart",
        "restart --wait ready --wait-stable 20",
        "resize 80x24",
        "resize 80x24 --wait-stable",
        "close",
        "close --all",
        "close --all --bypass-session",
        "close --bypass-session",
        "--tab api close",
    ]), @r##"
        exec 'ls -la' -> {"check":false,"command":"exec","env":null,"line":"ls -la","session":"work","tab":null,"timeout_ms":25000}
        exec --check true -> {"check":true,"command":"exec","env":null,"line":"true","session":"work","tab":null,"timeout_ms":25000}
        history -> {"command":"history","env":null,"failed":false,"last":null,"session":"work","tab":null}
        history --last 3 --failed -> {"command":"history","env":null,"failed":true,"last":3,"session":"work","tab":null}
        history show 4 -> {"command":"history-show","env":null,"id":4,"session":"work","tab":null}
        restart -> {"command":"restart","env":null,"session":"work","tab":null,"timeout_ms":25000,"wait":{"quiet_ms":300,"until":"stable"}}
        restart --wait ready --wait-stable 20 -> usage: ArgumentConflict
        resize 80x24 -> {"command":"resize","env":null,"session":"work","size":{"cols":80,"rows":24},"tab":null,"timeout_ms":25000,"wait":{"quiet_ms":300,"until":"stable"}}
        resize 80x24 --wait-stable -> {"command":"resize","env":null,"session":"work","size":{"cols":80,"rows":24},"tab":null,"timeout_ms":25000,"wait":{"quiet_ms":300,"until":"stable"}}
        close -> {"command":"close","env":null,"scope":"tab","session":"work","tab":null}
        close --all -> {"command":"close","env":null,"scope":"session","session":"work"}
        close --all --bypass-session -> refused: the argument '--bypass-session' cannot be used with '--session <NAME>': close --all --session <name> closes the tabs of that session, close --all --bypass-session closes every tab in every session
        close --bypass-session -> usage: MissingRequiredArgument
        --tab api close -> {"command":"close","env":null,"scope":"tab","session":"work","tab":"api"}
    "##);
}

#[test]
fn reading_commands_ask_for_the_screen_or_its_output() {
    insta::assert_snapshot!(table(&[
        "snapshot",
        "snapshot --rows 2:5 --scrollback --ansi -i",
        "snapshot --scrollback 10",
        "snapshot -s",
        "find foo",
        "find 'a.c' --regex --all",
        "mark build",
        "output",
        "output --since build --raw",
        "diff snapshot",
        "screenshot",
        "screenshot shot.svg --annotate --scale 2 --theme light --rows 1:3 --no-cursor",
        "screenshot shot.png --format html --cursor",
        "trace export /tmp/t.zip",
    ]), @r##"
        snapshot -> {"command":"snapshot","env":null,"options":{"ansi":false,"refs":false,"rows":null,"scrollback":"none","squeeze_blank":false},"session":"work","tab":null}
        snapshot --rows 2:5 --scrollback --ansi -i -> {"command":"snapshot","env":null,"options":{"ansi":true,"refs":true,"rows":{"first":2,"last":5},"scrollback":"all","squeeze_blank":false},"session":"work","tab":null}
        snapshot --scrollback 10 -> {"command":"snapshot","env":null,"options":{"ansi":false,"refs":false,"rows":null,"scrollback":{"last":10},"squeeze_blank":false},"session":"work","tab":null}
        snapshot -s -> {"command":"snapshot","env":null,"options":{"ansi":false,"refs":false,"rows":null,"scrollback":"none","squeeze_blank":true},"session":"work","tab":null}
        find foo -> {"all":false,"command":"find","env":null,"regex":false,"session":"work","tab":null,"text":"foo"}
        find 'a.c' --regex --all -> {"all":true,"command":"find","env":null,"regex":true,"session":"work","tab":null,"text":"a.c"}
        mark build -> {"command":"mark","env":null,"name":"build","session":"work","tab":null}
        output -> {"command":"output","env":null,"raw":false,"session":"work","since":null,"tab":null}
        output --since build --raw -> {"command":"output","env":null,"raw":true,"session":"work","since":"build","tab":null}
        diff snapshot -> {"baseline":null,"command":"diff-snapshot","env":null,"session":"work","tab":null}
        screenshot -> {"command":"screenshot","env":null,"session":"work","shot":{"annotate":false,"dir":"/shots","format":"png","look":{"cursor":true,"scale":1,"theme":"dark"},"path":null,"rows":null},"tab":null}
        screenshot shot.svg --annotate --scale 2 --theme light --rows 1:3 --no-cursor -> {"command":"screenshot","env":null,"session":"work","shot":{"annotate":true,"dir":"/shots","format":"svg","look":{"cursor":false,"scale":2,"theme":"light"},"path":"<here>/shot.svg","rows":{"first":1,"last":3}},"tab":null}
        screenshot shot.png --format html --cursor -> {"command":"screenshot","env":null,"session":"work","shot":{"annotate":false,"dir":"/shots","format":"html","look":{"cursor":true,"scale":1,"theme":"dark"},"path":"<here>/shot.png","rows":null},"tab":null}
        trace export /tmp/t.zip -> {"command":"trace-export","env":null,"path":"/tmp/t.zip","session":"work"}
    "##);
}

#[test]
fn get_asks_for_one_fact() {
    insta::assert_snapshot!(table(&[
        "get text",
        "get cursor",
        "get size",
        "get frame",
        "get program",
        "get exit-code",
        "get pid",
        "get title",
        "get cwd",
        "get modes",
        "get cell 4 3",
    ]), @r##"
        get text -> {"command":"get","env":null,"session":"work","tab":null,"what":"text"}
        get cursor -> {"command":"get","env":null,"session":"work","tab":null,"what":"cursor"}
        get size -> {"command":"get","env":null,"session":"work","tab":null,"what":"size"}
        get frame -> {"command":"get","env":null,"session":"work","tab":null,"what":"frame"}
        get program -> {"command":"get","env":null,"session":"work","tab":null,"what":"program"}
        get exit-code -> {"command":"get","env":null,"session":"work","tab":null,"what":"exit-code"}
        get pid -> {"command":"get","env":null,"session":"work","tab":null,"what":"pid"}
        get title -> {"command":"get","env":null,"session":"work","tab":null,"what":"title"}
        get cwd -> {"command":"get","env":null,"session":"work","tab":null,"what":"cwd"}
        get modes -> {"command":"get","env":null,"session":"work","tab":null,"what":"modes"}
        get cell 4 3 -> {"command":"get","env":null,"session":"work","tab":null,"what":{"cell":{"col":3,"row":4}}}
    "##);
}

#[test]
fn typing_and_keys_send_their_input_then_wait() {
    insta::assert_snapshot!(table(&[
        "type hello",
        "type hi --delay 20 --wait done --snapshot",
        "type hi --wait-gone busy --screenshot",
        "type hi --screenshot out.png",
        "type hi --wait-stable",
        "press Enter",
        "press Ctrl+c Up --repeat 3",
        "press NotAKey",
        "press Space --wait-stable --snapshot --rows 5:14 -i -s",
        "press Space --snapshot --scrollback 3 --ansi",
        "press Enter --rows 2:3",
        "paste 'a b'",
    ]), @r##"
        type hello -> {"command":"input","env":null,"input":{"delay_ms":0,"kind":"type","text":"hello"},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":null}
        type hi --delay 20 --wait done --snapshot -> {"command":"input","env":null,"input":{"delay_ms":20,"kind":"type","text":"hi"},"screenshot":null,"session":"work","snapshot":{"ansi":false,"refs":false,"rows":null,"scrollback":"none","squeeze_blank":false},"tab":null,"timeout_ms":25000,"wait":{"regex":false,"rows":null,"text":"done","until":"text"}}
        type hi --wait-gone busy --screenshot -> {"command":"input","env":null,"input":{"delay_ms":0,"kind":"type","text":"hi"},"screenshot":{"annotate":false,"dir":"/shots","format":"png","look":{"cursor":true,"scale":1,"theme":"dark"},"path":null,"rows":null},"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":{"regex":false,"rows":null,"text":"busy","until":"gone"}}
        type hi --screenshot out.png -> {"command":"input","env":null,"input":{"delay_ms":0,"kind":"type","text":"hi"},"screenshot":{"annotate":false,"dir":"/shots","format":"png","look":{"cursor":true,"scale":1,"theme":"dark"},"path":"<here>/out.png","rows":null},"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":null}
        type hi --wait-stable -> {"command":"input","env":null,"input":{"delay_ms":0,"kind":"type","text":"hi"},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":{"quiet_ms":300,"until":"stable"}}
        press Enter -> {"command":"input","env":null,"input":{"keys":["Enter"],"kind":"press","repeat":1},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":null}
        press Ctrl+c Up --repeat 3 -> {"command":"input","env":null,"input":{"keys":["Ctrl+c","Up"],"kind":"press","repeat":3},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":null}
        press NotAKey -> usage: ValueValidation
        press Space --wait-stable --snapshot --rows 5:14 -i -s -> {"command":"input","env":null,"input":{"keys":["Space"],"kind":"press","repeat":1},"screenshot":null,"session":"work","snapshot":{"ansi":false,"refs":true,"rows":{"first":5,"last":14},"scrollback":"none","squeeze_blank":true},"tab":null,"timeout_ms":25000,"wait":{"quiet_ms":300,"until":"stable"}}
        press Space --snapshot --scrollback 3 --ansi -> {"command":"input","env":null,"input":{"keys":["Space"],"kind":"press","repeat":1},"screenshot":null,"session":"work","snapshot":{"ansi":true,"refs":false,"rows":null,"scrollback":{"last":3},"squeeze_blank":false},"tab":null,"timeout_ms":25000,"wait":null}
        press Enter --rows 2:3 -> usage: MissingRequiredArgument
        paste 'a b' -> {"command":"input","env":null,"input":{"kind":"paste","text":"a b"},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":null}
    "##);
}

#[test]
fn bytes_signals_and_the_mouse_send_their_input_then_wait() {
    insta::assert_snapshot!(table(&[
        "send '\\e[A\\x41'",
        "send '\\q'",
        "signal INT",
        "signal TERM --wait-stable 10",
        "click @t3",
        "click 4,3 --right --double",
        "click OK",
        "scroll up",
        "scroll down 5 --keys",
        "mouse move 2,3",
        "mouse down 2,3",
        "mouse up 2,3",
        "mouse wheel 2,3",
        "mouse wheel up 2,3",
    ]), @r##"
        send '\e[A\x41' -> {"command":"input","env":null,"input":{"bytes":[27,91,65,65],"kind":"send"},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":null}
        send '\q' -> usage: ValueValidation
        signal INT -> {"command":"input","env":null,"input":{"kind":"signal","signal":"INT"},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":null}
        signal TERM --wait-stable 10 -> {"command":"input","env":null,"input":{"kind":"signal","signal":"TERM"},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":{"quiet_ms":10,"until":"stable"}}
        click @t3 -> {"command":"input","env":null,"input":{"button":"left","double":false,"kind":"click","target":"@t3"},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":null}
        click 4,3 --right --double -> {"command":"input","env":null,"input":{"button":"right","double":true,"kind":"click","target":"4,3"},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":null}
        click OK -> {"command":"input","env":null,"input":{"button":"left","double":false,"kind":"click","target":"OK"},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":null}
        scroll up -> {"command":"input","env":null,"input":{"count":1,"direction":"up","keys":false,"kind":"scroll"},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":null}
        scroll down 5 --keys -> {"command":"input","env":null,"input":{"count":5,"direction":"down","keys":true,"kind":"scroll"},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":null}
        mouse move 2,3 -> {"command":"input","env":null,"input":{"event":{"col":3,"event":"move","row":2},"kind":"mouse"},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":null}
        mouse down 2,3 -> {"command":"input","env":null,"input":{"event":{"button":"left","col":3,"event":"down","row":2},"kind":"mouse"},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":null}
        mouse up 2,3 -> {"command":"input","env":null,"input":{"event":{"button":"left","col":3,"event":"up","row":2},"kind":"mouse"},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":null}
        mouse wheel 2,3 -> usage: ValueValidation
        mouse wheel up 2,3 -> {"command":"input","env":null,"input":{"event":{"col":3,"direction":"up","event":"wheel","row":2},"kind":"mouse"},"screenshot":null,"session":"work","snapshot":null,"tab":null,"timeout_ms":25000,"wait":null}
    "##);
}

#[test]
fn waiting_and_expecting_name_one_condition_and_its_options() {
    insta::assert_snapshot!(table(&[
        "wait ready",
        "wait 'r.*y' --regex --rows 1:2 --assert",
        "wait busy --gone",
        "wait 500",
        "wait --stable",
        "wait --stable 40",
        "wait --idle 30",
        "wait --exit --code 2",
        "wait --cursor 3,4",
        "wait --frame 9",
        "wait --command",
        "wait --control",
        "wait",
        "expect ready",
        "expect ready --gone",
        "expect --exit-code 0",
        "expect --cell 2,3 --style bold,fg=red",
        "expect --cell 2,3",
    ]), @r##"
        wait ready -> {"assert":false,"command":"wait","env":null,"session":"work","tab":null,"timeout_ms":25000,"wait":{"regex":false,"rows":null,"text":"ready","until":"text"}}
        wait 'r.*y' --regex --rows 1:2 --assert -> {"assert":true,"command":"wait","env":null,"session":"work","tab":null,"timeout_ms":25000,"wait":{"regex":true,"rows":{"first":1,"last":2},"text":"r.*y","until":"text"}}
        wait busy --gone -> {"assert":false,"command":"wait","env":null,"session":"work","tab":null,"timeout_ms":25000,"wait":{"regex":false,"rows":null,"text":"busy","until":"gone"}}
        wait 500 -> {"assert":false,"command":"wait","env":null,"session":"work","tab":null,"timeout_ms":25000,"wait":{"ms":500,"until":"elapsed"}}
        wait --stable -> {"assert":false,"command":"wait","env":null,"session":"work","tab":null,"timeout_ms":25000,"wait":{"quiet_ms":300,"until":"stable"}}
        wait --stable 40 -> {"assert":false,"command":"wait","env":null,"session":"work","tab":null,"timeout_ms":25000,"wait":{"quiet_ms":40,"until":"stable"}}
        wait --idle 30 -> {"assert":false,"command":"wait","env":null,"session":"work","tab":null,"timeout_ms":25000,"wait":{"quiet_ms":30,"until":"idle"}}
        wait --exit --code 2 -> {"assert":false,"command":"wait","env":null,"session":"work","tab":null,"timeout_ms":25000,"wait":{"code":2,"until":"exit"}}
        wait --cursor 3,4 -> {"assert":false,"command":"wait","env":null,"session":"work","tab":null,"timeout_ms":25000,"wait":{"col":4,"row":3,"until":"cursor"}}
        wait --frame 9 -> {"assert":false,"command":"wait","env":null,"session":"work","tab":null,"timeout_ms":25000,"wait":{"frame":9,"until":"frame"}}
        wait --command -> {"assert":false,"command":"wait","env":null,"session":"work","tab":null,"timeout_ms":25000,"wait":{"until":"command"}}
        wait --control -> {"assert":false,"command":"wait-control","env":null,"session":"work","tab":null,"timeout_ms":25000}
        wait -> usage: MissingRequiredArgument
        expect ready -> {"command":"expect","env":null,"expect":{"check":"text","regex":false,"rows":null,"text":"ready"},"session":"work","tab":null}
        expect ready --gone -> {"command":"expect","env":null,"expect":{"check":"gone","regex":false,"rows":null,"text":"ready"},"session":"work","tab":null}
        expect --exit-code 0 -> {"command":"expect","env":null,"expect":{"check":"exit-code","code":0},"session":"work","tab":null}
        expect --cell 2,3 --style bold,fg=red -> {"command":"expect","env":null,"expect":{"check":"cell","col":3,"row":2,"style":["bold",{"fg":"red"}]},"session":"work","tab":null}
        expect --cell 2,3 -> usage: MissingRequiredArgument
    "##);
}

#[test]
fn session_and_tool_commands_ask_the_daemon_about_itself() {
    insta::assert_snapshot!(table(&[
        "session list",
        "tab",
        "tab list",
        "tab api",
        "tab close api",
        "tab rename web",
        "--tab api tab rename web",
        "tab rename Web",
        "dashboard",
        "dashboard --port 0 --allow-origin http://localhost:5173",
        "dashboard stop",
    ]), @r##"
        session list -> {"command":"sessions","env":null}
        tab -> {"command":"tabs","env":null,"session":"work"}
        tab list -> {"command":"tabs","env":null,"session":"work"}
        tab api -> {"command":"switch-tab","env":null,"session":"work","tab":"api"}
        tab close api -> {"command":"close","env":null,"scope":"tab","session":"work","tab":"api"}
        tab rename web -> {"command":"rename-tab","env":null,"name":"web","session":"work","tab":null}
        --tab api tab rename web -> {"command":"rename-tab","env":null,"name":"web","session":"work","tab":"api"}
        tab rename Web -> usage: ValueValidation
        dashboard -> {"command":"dashboard-start","env":null,"origins":[],"port":4849}
        dashboard --port 0 --allow-origin http://localhost:5173 -> {"command":"dashboard-start","env":null,"origins":["http://localhost:5173"],"port":0}
        dashboard stop -> {"command":"dashboard-stop","env":null}
    "##);
}
