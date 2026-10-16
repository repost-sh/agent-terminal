use gherkin_cargo_test::{Ctx, DataTable};
use serde_json::Value;

use super::common::json;
use super::{Steps, arg};
use crate::programs;
use crate::terminal::Terminal;

pub(crate) fn define(steps: &mut Steps<'_>) {
    steps
        .given(r#"^terminals open in the sessions "([^"]*)" and "([^"]*)"$"#, open_in)
        .when(r"^the agent lists the sessions$", list)
        .when(r#"^the agent closes the tab in "([^"]*)"$"#, close)
        .when(r#"^the agent closes every tab in "([^"]*)"$"#, close_all)
        .when(r"^the agent opens a program without naming a session$", open_unnamed)
        .then(r#"^the error says to create the session first with "([^"]*)"$"#, says_to_create)
        .then(r#"^the list names exactly the sessions "([^"]*)" and "([^"]*)"$"#, names)
        .then(r#"^the list names exactly the sessions "([^"]*)"$"#, names);
}

fn open_in(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    match programs::fixture("line-printer") {
        Ok(printer) => {
            let printer = printer.to_string_lossy().into_owned();
            for session in args {
                ctx.world.setup(&["session", "new", session]);
                ctx.world.setup(&[
                    "--session",
                    session,
                    "open",
                    "--size",
                    "20x3",
                    "--wait",
                    "line 1",
                    &printer,
                    "--lines",
                    "1",
                    "--hold",
                ]);
            }
        }
        Err(failure) => ctx.world.failure = Some(failure),
    }
}

fn list(ctx: &mut Ctx<Terminal>, _: &[String], _: Option<&DataTable>) {
    ctx.world.act(&["--json", "session", "list"]);
}

fn close(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    ctx.world.act(&["--session", arg(args, 0), "close"]);
}

fn close_all(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    ctx.world.act(&["--session", arg(args, 0), "close", "--all"]);
}

fn open_unnamed(ctx: &mut Ctx<Terminal>, _: &[String], _: Option<&DataTable>) {
    ctx.world.act(&["open", "--size", "20x3", "cat"]);
}

fn says_to_create(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    let world = &ctx.world;
    let said = world.ran().is_some_and(|ran| ran.stderr.contains(arg(args, 0)));
    assert!(said, "{}", world.report());
}

fn names(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    let world = &ctx.world;
    let listed = world.ran().map(|ran| {
        let sessions = json(&ran.stdout);
        let mut names: Vec<String> = sessions
            .get("sessions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|session| session.get("session").and_then(Value::as_str).map(String::from))
            .collect();
        names.sort();
        names
    });
    assert_eq!(listed.as_deref(), Some(args), "{}", world.report());
}
