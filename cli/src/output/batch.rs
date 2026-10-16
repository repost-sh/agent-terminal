use std::fmt::{self, Write as _};

use serde_json::{Map, Value};

use super::replies::reply_text;
use super::{error_text, reply_data};
use crate::protocol::{BatchOutcome, Response, Squeeze};

pub(super) fn batch_json(outcomes: &[BatchOutcome]) -> serde_json::Result<Value> {
    outcomes.iter().map(batch_entry).collect()
}

fn batch_entry(outcome: &BatchOutcome) -> serde_json::Result<Value> {
    let mut entry = Map::new();
    entry.insert("args".into(), serde_json::to_value(&outcome.args)?);
    entry.insert("exit_code".into(), serde_json::to_value(outcome.response.as_ref().map(Response::exit_code))?);
    match &outcome.response {
        Some(Response::Ok(ok)) => {
            entry.insert("result".into(), reply_data(ok)?);
        }
        Some(Response::Err(error)) => {
            entry.insert("error".into(), serde_json::to_value(error)?);
        }
        None => {}
    }
    Ok(Value::Object(entry))
}

pub(super) fn batch_text(outcomes: &[BatchOutcome], squeeze: &Squeeze) -> String {
    let steps = match squeeze {
        Squeeze::PerStep(steps) => steps.as_slice(),
        Squeeze::Off | Squeeze::On => &[],
    };
    fmt::from_fn(|f| write_batch(f, outcomes, steps)).to_string()
}

fn write_batch(f: &mut fmt::Formatter<'_>, outcomes: &[BatchOutcome], steps: &[Squeeze]) -> fmt::Result {
    for (index, outcome) in outcomes.iter().enumerate() {
        if index > 0 {
            f.write_char('\n')?;
        }
        let call = format!("[{}] {}", index + 1, call_line(&outcome.args));
        let Some(response) = &outcome.response else {
            writeln!(f, "{call} · not run")?;
            continue;
        };
        writeln!(f, "{call} · exit {}", response.exit_code())?;
        match response {
            Response::Ok(ok) => f.write_str(&reply_text(ok, steps.get(index).unwrap_or(&Squeeze::Off)))?,
            Response::Err(error) => f.write_str(&error_text(error))?,
        }
    }
    Ok(())
}

pub(crate) fn call_line(args: &[String]) -> String {
    let words: Vec<String> = args
        .iter()
        .map(|arg| shlex::try_quote(arg).map_or_else(|_| format!("{arg:?}"), std::borrow::Cow::into_owned))
        .collect();
    words.join(" ")
}
