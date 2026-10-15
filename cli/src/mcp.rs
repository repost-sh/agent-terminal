use std::any::TypeId;
use std::future::ready;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result};
use clap::{Arg, ArgAction};
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ContentBlock, Implementation, JsonObject, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler, ServiceExt};
use serde_json::{Value, json};

use crate::output;
use crate::protocol::Scrollback;

const SERVER_COMMAND: &str = "mcp";
const JSON_FLAG: &str = "json";
const INSTRUCTIONS: &str = "Each tool runs the agent-terminal command of the same name with the same arguments \
     (get_text is `agent-terminal get text`) and returns what the command prints with --json. \
     Start with skills_get, name core, for the usage guide.";

pub(crate) fn run(cli: clap::Command) -> io::Result<i32> {
    match serve(cli) {
        Ok(()) => Ok(0),
        Err(e) => output::fail(&format!("{e:#}"), crate::exit::ERROR),
    }
}

fn serve(cli: clap::Command) -> Result<()> {
    let program = std::env::current_exe().context("find the agent-terminal binary")?;
    let server = Server { program, commands: commands(cli) };
    let runtime =
        tokio::runtime::Builder::new_current_thread().enable_all().build().context("start the MCP server runtime")?;
    runtime.block_on(async {
        let service = server.serve(rmcp::transport::stdio()).await.context("start the MCP session")?;
        service.waiting().await.context("run the MCP session")?;
        Ok(())
    })
}

struct Server {
    program: PathBuf,
    commands: Vec<ToolCommand>,
}

impl Server {
    fn command(&self, name: &str) -> Option<&ToolCommand> {
        self.commands.iter().find(|command| command.tool.name == name)
    }
}

impl ServerHandler for Server {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("agent-terminal", crate::VERSION))
            .with_instructions(INSTRUCTIONS)
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        ready(Ok(ListToolsResult::with_all_items(self.commands.iter().map(|command| command.tool.clone()).collect())))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.command(name).map(|command| command.tool.clone())
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let command = self
            .command(&request.name)
            .ok_or_else(|| ErrorData::invalid_params(format!("unknown tool: {}", request.name), None))?;
        match command.args(&request.arguments.unwrap_or_default()) {
            Ok(args) => Ok(run_command(&self.program, &args).await),
            Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
        }
    }
}

async fn run_command(program: &Path, args: &[String]) -> CallToolResult {
    let ran = tokio::process::Command::new(program).args(args).stdin(Stdio::null()).kill_on_drop(true).output().await;
    let output = match ran {
        Ok(output) => output,
        Err(e) => return CallToolResult::error(vec![ContentBlock::text(format!("run {}: {e}", program.display()))]),
    };
    let mut content: Vec<ContentBlock> = [&output.stdout, &output.stderr]
        .into_iter()
        .map(|printed| String::from_utf8_lossy(printed).trim_end().to_string())
        .filter(|printed| !printed.is_empty())
        .map(ContentBlock::text)
        .collect();
    if output.status.success() {
        return CallToolResult::success(content);
    }
    if content.is_empty() {
        content.push(ContentBlock::text(format!("agent-terminal ended with {}", output.status)));
    }
    CallToolResult::error(content)
}

struct ToolCommand {
    tool: Tool,
    path: Vec<String>,
    params: Vec<Param>,
}

struct Param {
    name: String,
    form: Form,
}

enum Form {
    Switch { flag: String },
    Count { flag: String },
    Value { flag: String, many: bool, bare: bool },
    Positional { many: bool },
}

fn commands(cli: clap::Command) -> Vec<ToolCommand> {
    let mut cli = cli.disable_help_subcommand(true);
    cli.build();
    let mut commands = Vec::new();
    for command in cli.get_subcommands().filter(|command| command.get_name() != SERVER_COMMAND) {
        collect(command, Vec::new(), &mut commands);
    }
    commands
}

fn collect(command: &clap::Command, mut path: Vec<String>, commands: &mut Vec<ToolCommand>) {
    if command.is_hide_set() {
        return;
    }
    path.push(command.get_name().to_string());
    if !command.is_subcommand_required_set() {
        commands.push(ToolCommand::new(command, path.clone()));
    }
    for subcommand in command.get_subcommands() {
        collect(subcommand, path.clone(), commands);
    }
}

impl ToolCommand {
    fn new(command: &clap::Command, path: Vec<String>) -> Self {
        let mut properties = JsonObject::new();
        let mut required = Vec::new();
        let mut params = Vec::new();
        let args = command.get_arguments().filter(|arg| !arg.is_hide_set() && arg.get_id() != JSON_FLAG);
        for (arg, form) in args.filter_map(|arg| Some((arg, form(arg)?))) {
            let name = arg.get_id().to_string();
            properties.insert(name.clone(), schema(arg, &form));
            if arg.is_required_set() {
                required.push(Value::String(name.clone()));
            }
            params.push(Param { name, form });
        }
        let mut schema = JsonObject::new();
        schema.insert("type".into(), "object".into());
        schema.insert("properties".into(), Value::Object(properties));
        schema.insert("required".into(), Value::Array(required));
        schema.insert("additionalProperties".into(), false.into());
        let name = path.join("_").replace('-', "_");
        let about = command.get_about().map(|about| about.to_string().into());
        Self { tool: Tool::new_with_raw(name, about, schema), path, params }
    }

    fn args(&self, arguments: &JsonObject) -> Result<Vec<String>, String> {
        if let Some(unknown) = arguments.keys().find(|key| !self.params.iter().any(|param| &param.name == *key)) {
            let known: Vec<&str> = self.params.iter().map(|param| param.name.as_str()).collect();
            return Err(format!("{} takes no argument `{unknown}`; it takes {}", self.tool.name, known.join(", ")));
        }
        let mut words = Words { options: self.path.clone(), positionals: Vec::new() };
        for param in &self.params {
            let Some(value) = arguments.get(&param.name).filter(|value| !value.is_null()) else {
                continue;
            };
            param.add(value, &mut words)?;
        }
        let Words { mut options, positionals } = words;
        options.push(format!("--{JSON_FLAG}"));
        if !positionals.is_empty() {
            options.push("--".into());
            options.extend(positionals);
        }
        Ok(options)
    }
}

struct Words {
    options: Vec<String>,
    positionals: Vec<String>,
}

impl Param {
    fn add(&self, value: &Value, words: &mut Words) -> Result<(), String> {
        let refuse = |expected: &str| format!("`{}` expects {expected}, got {value}", self.name);
        let each = |items: &[Value]| {
            items
                .iter()
                .map(|item| scalar(item).ok_or_else(|| refuse("a list of values")))
                .collect::<Result<Vec<_>, _>>()
        };
        match (&self.form, value) {
            (Form::Switch { flag } | Form::Value { flag, bare: true, .. }, Value::Bool(true)) => {
                words.options.push(flag.clone());
            }
            (Form::Switch { .. } | Form::Value { bare: true, .. }, Value::Bool(false)) => {}
            (Form::Switch { .. }, _) => return Err(refuse("true or false")),
            (Form::Count { flag }, value) => {
                let times = value.as_u64().ok_or_else(|| refuse("a count"))?;
                words.options.extend((0..times).map(|_| flag.clone()));
            }
            (Form::Value { flag, many: true, .. }, Value::Array(items)) => {
                words.options.extend(each(items)?.into_iter().map(|item| format!("{flag}={item}")));
            }
            (Form::Value { flag, .. }, value) => {
                words.options.push(format!("{flag}={}", scalar(value).ok_or_else(|| refuse("a value"))?));
            }
            (Form::Positional { many: true }, Value::Array(items)) => words.positionals.extend(each(items)?),
            (Form::Positional { .. }, value) => words.positionals.push(scalar(value).ok_or_else(|| refuse("a value"))?),
        }
        Ok(())
    }
}

fn form(arg: &Arg) -> Option<Form> {
    let many =
        matches!(arg.get_action(), ArgAction::Append) || arg.get_num_args().is_some_and(|range| range.max_values() > 1);
    if arg.is_positional() {
        return matches!(arg.get_action(), ArgAction::Set | ArgAction::Append).then_some(Form::Positional { many });
    }
    let flag =
        arg.get_long().map(|long| format!("--{long}")).or_else(|| arg.get_short().map(|short| format!("-{short}")))?;
    match arg.get_action() {
        ArgAction::SetTrue => Some(Form::Switch { flag }),
        ArgAction::Count => Some(Form::Count { flag }),
        ArgAction::Set | ArgAction::Append => {
            let bare = arg.get_num_args().is_some_and(|range| range.min_values() == 0);
            Some(Form::Value { flag, many, bare })
        }
        _ => None,
    }
}

fn schema(arg: &Arg, form: &Form) -> Value {
    let description = description(arg, form);
    match form {
        Form::Switch { .. } => json!({ "type": "boolean", "description": description }),
        Form::Count { .. } => json!({ "type": "integer", "minimum": 0, "description": description }),
        Form::Value { many: true, .. } | Form::Positional { many: true } => {
            json!({ "type": "array", "items": value_schema(arg), "description": description })
        }
        Form::Value { bare: true, .. } => {
            json!({ "anyOf": [value_schema(arg), { "type": "boolean" }], "description": description })
        }
        Form::Value { .. } | Form::Positional { .. } => {
            let mut schema = value_schema(arg);
            schema.insert("description".into(), Value::String(description));
            Value::Object(schema)
        }
    }
}

fn choices(arg: &Arg) -> Vec<String> {
    arg.get_possible_values().iter().filter(|value| !value.is_hide_set()).map(|value| value.get_name().into()).collect()
}

fn value_schema(arg: &Arg) -> JsonObject {
    let choices = choices(arg);
    let mut schema = JsonObject::new();
    if choices.is_empty() {
        schema.insert("type".into(), (if is_integer(arg) { "integer" } else { "string" }).into());
    } else {
        schema.insert("type".into(), "string".into());
        schema.insert("enum".into(), choices.into());
    }
    schema
}

fn is_integer(arg: &Arg) -> bool {
    let parsed = arg.get_value_parser().type_id();
    [
        TypeId::of::<u8>(),
        TypeId::of::<u16>(),
        TypeId::of::<u32>(),
        TypeId::of::<u64>(),
        TypeId::of::<usize>(),
        TypeId::of::<i8>(),
        TypeId::of::<i16>(),
        TypeId::of::<i32>(),
        TypeId::of::<i64>(),
        TypeId::of::<isize>(),
        TypeId::of::<Scrollback>(),
    ]
    .iter()
    .any(|integer| parsed == *integer)
}

fn description(arg: &Arg, form: &Form) -> String {
    let help = arg.get_help().map_or_else(String::new, ToString::to_string);
    let takes_text = matches!(form, Form::Value { .. } | Form::Positional { .. }) && !is_integer(arg);
    let value = arg
        .get_value_names()
        .filter(|_| takes_text && choices(arg).is_empty())
        .map(|names| {
            let names: Vec<String> = names.iter().map(ToString::to_string).collect();
            format!(" [value: {}]", names.join(" "))
        })
        .unwrap_or_default();
    let bare = if let Form::Value { bare: true, .. } = form { " [or true: the option without a value]" } else { "" };
    let switch = matches!(form, Form::Switch { .. });
    let default = if !switch && !arg.get_default_values().is_empty() && arg.get_env().is_none() {
        format!(" [default: {}]", listed(arg.get_default_values()))
    } else {
        String::new()
    };
    format!("{help}{value}{bare}{default}")
}

fn listed(values: &[clap::builder::OsStr]) -> String {
    values.iter().map(|value| value.to_string_lossy()).collect::<Vec<_>>().join(",")
}

fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}
