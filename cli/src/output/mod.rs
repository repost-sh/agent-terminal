mod batch;
mod delivery;
mod facts;
mod records;
mod replies;
mod saved_screen;
mod screen;

use std::io::{self, ErrorKind, Write};

use serde::Serialize;
use serde_json::Value;

use batch::batch_json;
pub(crate) use batch::call_line;
pub(crate) use replies::reply_text;
pub(crate) use saved_screen::saved_rows;
pub(crate) use screen::screen;

use crate::protocol::{ErrorBody, Reply};
use crate::session_id::SessionId;

pub(crate) fn print(text: &str) -> io::Result<()> {
    unless_the_reader_left(write_all(&mut io::stdout().lock(), text))
}

pub(crate) fn print_error(text: &str) -> io::Result<()> {
    unless_the_reader_left(write_all(&mut io::stderr().lock(), text))
}

pub(crate) fn print_json(value: &impl Serialize) -> io::Result<()> {
    let value = serde_json::to_value(value)?;
    print(&format!("{value}\n"))
}

fn write_all(stream: &mut impl Write, text: &str) -> io::Result<()> {
    stream.write_all(text.as_bytes())?;
    stream.flush()
}

fn unless_the_reader_left(written: io::Result<()>) -> io::Result<()> {
    match written {
        Err(e) if e.kind() == ErrorKind::BrokenPipe => Ok(()),
        written => written,
    }
}

pub(crate) fn reply(reply: &Reply, json: bool) -> io::Result<()> {
    if !json {
        return print(&reply_text(reply));
    }
    match reply {
        Reply::Stopped | Reply::KeptRunning(_) => Ok(()),
        Reply::Batch(outcomes) => print_json(&batch_json(outcomes)?),
        reply => print_json(&reply_data(reply)?),
    }
}

pub(super) fn reply_data(reply: &Reply) -> serde_json::Result<Value> {
    let mut whole = serde_json::to_value(reply)?;
    Ok(whole.get_mut("data").map(Value::take).unwrap_or_default())
}

pub(crate) fn session(name: &str, json: bool) -> io::Result<()> {
    if json { print_json(&serde_json::json!({ "session": name })) } else { print(&format!("{name}\n")) }
}

pub(crate) fn session_id(id: &SessionId, json: bool) -> io::Result<()> {
    if json { print_json(id) } else { print(&format!("{}\n", id.session)) }
}

pub(crate) fn count(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {noun}s") }
}

#[derive(Serialize)]
struct Failed<'a> {
    error: &'a ErrorBody,
}

pub(crate) fn error(error: &ErrorBody, json: bool) -> io::Result<i32> {
    if json {
        print_json(&Failed { error })?;
    } else {
        print_error(&error_text(error))?;
    }
    Ok(error.code)
}

pub(crate) fn error_text(error: &ErrorBody) -> String {
    let shown = error.snapshot.as_deref().map(screen).unwrap_or_default();
    format!("error: {}\n{shown}", error.message)
}

pub(crate) fn fail(message: &str, code: i32) -> io::Result<i32> {
    print_error(&format!("error: {message}\n"))?;
    Ok(code)
}

pub(crate) fn refuse(message: &str, json: bool) -> io::Result<i32> {
    if json {
        return error(&ErrorBody::failure(message.trim_end()), true);
    }
    fail(message, crate::exit::ERROR)
}

pub(crate) fn version(json: bool) -> io::Result<i32> {
    if json {
        print_json(&serde_json::json!({ "version": crate::VERSION }))?;
    } else {
        print(&format!("agent-terminal {}\n", crate::VERSION))?;
    }
    Ok(0)
}
