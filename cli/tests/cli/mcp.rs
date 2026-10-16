use std::collections::{BTreeMap, BTreeSet};

use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult, Tool};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::TokioChildProcess;
use serde_json::{Value, json};

use crate::support::{AgentTerminal, fixture};

type Client = RunningService<RoleClient, ()>;

async fn connect(at: &AgentTerminal, env: &[(&str, &str)]) -> Client {
    let mut command = at.command(&["mcp"]);
    command.envs(env.iter().copied());
    let transport = TokioChildProcess::new(tokio::process::Command::from(command)).expect("start agent-terminal mcp");
    ().serve(transport).await.expect("initialize an MCP session with agent-terminal mcp")
}

async fn call(client: &Client, tool: &str, arguments: Value) -> CallToolResult {
    let Value::Object(arguments) = arguments else { panic!("tool arguments are a JSON object") };
    client
        .call_tool(CallToolRequestParams::new(tool.to_string()).with_arguments(arguments))
        .await
        .unwrap_or_else(|e| panic!("call {tool}: {e}"))
}

fn text(result: &CallToolResult) -> String {
    result.content.iter().filter_map(|content| content.as_text()).map(|text| text.text.as_str()).collect()
}

fn json_of(result: &CallToolResult) -> Value {
    let text = text(result);
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("the tool result is not JSON ({e}):\n{text}"))
}

struct Help {
    about: String,
    runs_on_its_own: bool,
    subcommands: Vec<String>,
    options: BTreeSet<String>,
}

#[derive(Default)]
struct HelpText {
    section: String,
    usages: Vec<String>,
    subcommands: Vec<String>,
    options: BTreeSet<String>,
}

impl HelpText {
    fn read(&mut self, line: &str) {
        if let Some(usage) = line.strip_prefix("Usage: ") {
            self.section = "Usage".to_string();
            self.usages.push(usage.to_string());
        } else if !line.starts_with(' ') && line.ends_with(':') {
            self.section = line.to_string();
        } else {
            self.read_entry(line);
        }
    }

    fn read_entry(&mut self, line: &str) {
        match self.section.as_str() {
            "Usage" if line.starts_with("       agent-terminal") => self.usages.push(line.trim().to_string()),
            "Commands:" => self.read_subcommand(line),
            "Options:" if line.trim_start().starts_with('-') => self.read_option(line),
            _ => {}
        }
    }

    fn read_subcommand(&mut self, line: &str) {
        let name = line.split_whitespace().next().expect("a command line names a command");
        if name != "help" {
            self.subcommands.push(name.to_string());
        }
    }

    fn read_option(&mut self, line: &str) {
        let long = line.split_whitespace().find_map(|word| word.strip_prefix("--"));
        if let Some(long) = long.filter(|long| !["help", "version", "json"].contains(long)) {
            self.options.insert(long.replace('-', "_"));
        }
    }
}

fn help(at: &AgentTerminal, path: &[String]) -> Help {
    let mut args: Vec<&str> = path.iter().map(String::as_str).collect();
    args.push("-h");
    let text = at.run(&args).success().stdout();
    let mut read = HelpText::default();
    for line in text.lines().filter(|line| !line.is_empty()) {
        read.read(line);
    }
    Help {
        about: text.lines().next().unwrap_or_default().to_string(),
        runs_on_its_own: read.usages.iter().any(|usage| !usage.contains("<COMMAND>")),
        subcommands: read.subcommands,
        options: read.options,
    }
}

fn commands(at: &AgentTerminal) -> BTreeMap<String, Help> {
    let mut found = BTreeMap::new();
    let mut pending: Vec<Vec<String>> =
        help(at, &[]).subcommands.into_iter().filter(|name| name != "mcp").map(|name| vec![name]).collect();
    while let Some(path) = pending.pop() {
        let command = help(at, &path);
        for sub in &command.subcommands {
            pending.push([path.clone(), vec![sub.clone()]].concat());
        }
        if command.runs_on_its_own {
            found.insert(path.join("_").replace('-', "_"), command);
        }
    }
    found
}

fn properties(tool: &Tool) -> BTreeSet<String> {
    tool.input_schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| properties.keys().cloned().collect())
        .unwrap_or_default()
}

#[tokio::test]
async fn every_command_of_the_cli_is_a_tool_with_its_options() {
    let at = AgentTerminal::new();
    let commands = commands(&at);
    for expected in
        ["open", "press", "snapshot", "get_text", "get_cell", "tab", "tab_new", "session_list", "skills_get"]
    {
        assert!(commands.contains_key(expected), "the help walk missed `{expected}`: {:?}", commands.keys());
    }

    let client = connect(&at, &[]).await;
    let tools: BTreeMap<String, Tool> = client
        .list_all_tools()
        .await
        .expect("list tools")
        .into_iter()
        .map(|tool| (tool.name.to_string(), tool))
        .collect();

    assert_eq!(tools.keys().collect::<Vec<_>>(), commands.keys().collect::<Vec<_>>());
    for (name, command) in &commands {
        let tool = &tools[name];
        assert_eq!(tool.description.as_deref(), Some(command.about.as_str()), "description of {name}");
        let properties = properties(tool);
        let missing: Vec<_> = command.options.difference(&properties).collect();
        assert!(missing.is_empty(), "tool {name} lacks the options {missing:?}: {:#}", json!(tool.input_schema));
    }
}

#[tokio::test]
async fn an_mcp_client_opens_the_fixture_tui_presses_a_key_with_a_wait_and_reads_a_snapshot() {
    let at = AgentTerminal::new();
    let client = connect(&at, &[]).await;

    let opened =
        call(&client, "open", json!({ "command": [fixture("fixture-tui")], "size": "50x8", "wait": "Quit: q" })).await;
    assert_eq!(opened.is_error, Some(false), "{}", text(&opened));
    let opened = json_of(&opened);
    assert_eq!(
        (&opened["session"], &opened["tab"], &opened["program"]),
        (&json!("work"), &json!("t1"), &json!("fixture-tui"))
    );

    let pressed = call(&client, "press", json!({ "keys": ["j"], "wait": "Selected: beta.txt" })).await;
    assert_eq!(pressed.is_error, Some(false), "{}", text(&pressed));
    assert_eq!(json_of(&pressed), json!({ "session": "work", "tab": "t1", "input": "keys", "keys": 1, "bytes": 1 }));

    let snapshot = json_of(&call(&client, "snapshot", json!({})).await);
    assert_eq!(
        snapshot["lines"],
        json!([
            "┌Files─────────────┐┌Details─────────────────────┐",
            "│alpha.txt         ││Selected: beta.txt          │",
            "│beta.txt          ││Size: 50x8                  │",
            "│gamma.txt         ││                            │",
            "│delta.txt         ││                            │",
            "│                  ││                            │",
            "└──────────────────┘└────────────────────────────┘",
            "Move: j/k | Quit: q | Crash: x",
        ])
    );
    assert_eq!(snapshot, at.run(&["snapshot", "--json"]).success().json());
}

#[tokio::test]
async fn tools_take_a_session_and_default_to_agent_terminal_session() {
    let at = AgentTerminal::new();
    let client = connect(&at, &[("AGENT_TERMINAL_SESSION", "from-env")]).await;

    let current = call(&client, "session", json!({})).await;
    assert_eq!(json_of(&current), json!({ "session": "from-env" }));
    for name in ["from-env", "named"] {
        let created = call(&client, "session_new", json!({ "name": name })).await;
        assert_eq!(json_of(&created), json!({ "session": name }), "{}", text(&created));
    }

    let named =
        call(&client, "open", json!({ "session": "named", "command": [fixture("line-printer"), "--hold"] })).await;
    assert_eq!(json_of(&named)["session"], "named", "{}", text(&named));
    let unnamed = call(&client, "open", json!({ "command": [fixture("line-printer"), "--hold"] })).await;
    assert_eq!(json_of(&unnamed)["session"], "from-env", "{}", text(&unnamed));

    let sessions = at.run(&["session", "list", "--json"]).success().json();
    let listed: Vec<&Value> =
        sessions["sessions"].as_array().unwrap().iter().map(|session| &session["session"]).collect();
    assert_eq!(listed, [&json!("from-env"), &json!("named"), &json!("work")]);
}

#[tokio::test]
async fn the_tab_property_takes_a_tab_id_like_t2() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "20x2", "--wait", "line 1", &fixture("line-printer"), "--lines", "1", "--hold"])
        .success();
    at.run(&["tab", "new", "--size", "30x3", "--wait", "line 2", &fixture("line-printer"), "--lines", "2", "--hold"])
        .success();
    at.run(&["tab", "t1"]).success();
    let client = connect(&at, &[]).await;

    let tools = client.list_all_tools().await.expect("list the tools");
    let snapshot = tools.iter().find(|tool| tool.name == "snapshot").expect("a snapshot tool");
    assert_eq!(snapshot.input_schema["properties"]["tab"]["type"], "string");

    let second = call(&client, "snapshot", json!({ "tab": "t2" })).await;
    assert_eq!((json_of(&second)["tab"].clone(), json_of(&second)["cols"].clone()), (json!("t2"), json!(30)));
}

#[tokio::test]
async fn a_failing_command_is_a_tool_error_carrying_the_clis_json_error() {
    let at = AgentTerminal::new();
    at.run(&["open", "--wait", "line 2", &fixture("line-printer"), "--lines", "2", "--hold"]).success();
    let client = connect(&at, &[]).await;

    let checked = call(&client, "expect", json!({ "text": "line 9" })).await;
    assert_eq!(checked.is_error, Some(true));
    let error = json_of(&checked);
    assert_eq!(error["error"]["code"], 2);
    assert_eq!(error["error"]["message"], "\"line 9\" is not on screen in work t1");

    let cli = at.run(&["expect", "line 9", "--json"]);
    assert_eq!(cli.code(), 2);
    assert_eq!(error, cli.json());
}

#[tokio::test]
async fn options_without_a_value_and_switches_take_true() {
    let at = AgentTerminal::new();
    at.run(&["open", "--size", "40x4", "--wait", "line 6", &fixture("line-printer"), "--lines", "6", "--hold"])
        .success();
    let client = connect(&at, &[]).await;

    let snapshot = json_of(&call(&client, "snapshot", json!({ "scrollback": true })).await);
    assert_eq!(snapshot["scrollback"], json!(["line 1", "line 2", "line 3"]));
    assert_eq!(snapshot, at.run(&["snapshot", "--scrollback", "--json"]).success().json());

    let found = json_of(&call(&client, "find", json!({ "text": "line [12]", "regex": true, "all": true })).await);
    assert_eq!(found, at.run(&["find", "line [12]", "--regex", "--all", "--json"]).success().json());
}

#[tokio::test]
async fn arguments_the_command_does_not_take_are_refused() {
    let at = AgentTerminal::new();
    let client = connect(&at, &[]).await;

    let unknown = call(&client, "snapshot", json!({ "colour": true })).await;
    assert_eq!(unknown.is_error, Some(true));
    assert!(text(&unknown).starts_with("snapshot takes no argument `colour`; it takes "), "{}", text(&unknown));

    let missing = call(&client, "open", json!({ "size": "80x24" })).await;
    assert_eq!(missing.is_error, Some(true));
    assert!(text(&missing).contains("the following required arguments were not provided"), "{}", text(&missing));
}

#[tokio::test]
async fn the_attach_tool_says_it_needs_a_terminal_and_what_to_use_instead() {
    let at = AgentTerminal::new();
    at.run(&["open", "--wait", "line 1", &fixture("line-printer"), "--lines", "1", "--hold"]).success();
    let client = connect(&at, &[]).await;

    let attached = call(&client, "attach", json!({ "read_only": true })).await;
    assert_eq!(attached.is_error, Some(true));
    assert_eq!(
        json_of(&attached),
        json!({ "error": {
            "code": 1,
            "message": "attach shows the tab in your terminal, but stdin or stdout is not a terminal; read the screen with `agent-terminal snapshot` instead"
        } })
    );
}
