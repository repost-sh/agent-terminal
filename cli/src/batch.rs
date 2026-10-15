use std::io::{IsTerminal, Read};

use serde::Deserialize;

const NO_COMMANDS: &str = "batch needs commands: pass each one as a quoted argument, or pipe a JSON array to stdin";

#[derive(Deserialize)]
#[serde(untagged)]
enum Call {
    Line(String),
    Args(Vec<String>),
}

pub(crate) fn calls(commands: &[String]) -> Result<Vec<Vec<String>>, String> {
    let calls = if commands.is_empty() { from_stdin()? } else { commands.iter().cloned().map(Call::Line).collect() };
    if calls.is_empty() {
        return Err(NO_COMMANDS.into());
    }
    calls
        .into_iter()
        .enumerate()
        .map(|(index, call)| match call {
            Call::Args(args) => Ok(args),
            Call::Line(line) => {
                shlex::split(&line).ok_or_else(|| not_valid(index, &line, "a quote or escape is not closed"))
            }
        })
        .collect()
}

pub(crate) fn not_valid(index: usize, call: &str, reason: &str) -> String {
    format!("batch command {} is not valid, so nothing ran: {call}\n{reason}", index + 1)
}

fn from_stdin() -> Result<Vec<Call>, String> {
    let mut stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Err(NO_COMMANDS.into());
    }
    let mut input = String::new();
    stdin.read_to_string(&mut input).map_err(|e| format!("read the commands from stdin: {e}"))?;
    if input.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&input)
        .map_err(|e| format!("stdin is not a JSON array of commands, like [[\"open\", \"htop\"], \"snapshot\"]: {e}"))
}
