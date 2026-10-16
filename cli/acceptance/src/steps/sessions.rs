use gherkin_cargo_test::{Ctx, DataTable, StepRegistry};
use serde_json::Value;

use super::common::{self, json};
use super::{Steps, arg};
use crate::programs;
use crate::terminal::Terminal;

pub(crate) fn define(registry: &mut StepRegistry<Terminal>) {
    let mut steps = Steps(registry);
    common::define(&mut steps);
    steps
        .given(r#"^terminals open in the sessions "([^"]*)" and "([^"]*)"$"#, open_in)
        .when(r"^the agent lists the sessions$", list)
        .when(r#"^the agent closes the tab in "([^"]*)"$"#, close)
        .then(r#"^the list names exactly the sessions "([^"]*)" and "([^"]*)"$"#, names)
        .then(r#"^the list names exactly the sessions "([^"]*)"$"#, names);
}

fn open_in(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    match programs::fixture("line-printer") {
        Ok(printer) => {
            let printer = printer.to_string_lossy().into_owned();
            for session in args {
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
