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
        .given(r#"^tabs named "([^"]*)" and "([^"]*)" are open$"#, open_tabs)
        .when(r"^the agent opens a program without naming a session$", open_unnamed)
        .when(r"^the agent opens a program without naming its tab$", open_without_a_tab_name)
        .when(r#"^the agent renames the tab "([^"]*)" to "([^"]*)"$"#, rename)
        .when(r#"^the agent reads the screen of the tab "([^"]*)"$"#, snapshot_of)
        .then(r#"^the error says to create the session first with "([^"]*)"$"#, error_says)
        .then(r#"^the error says to name the tab with "([^"]*)"$"#, error_says)
        .then(r#"^the screen is the tab "([^"]*)"$"#, screen_of)
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
                    "--name",
                    "printer",
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

fn open_tabs(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    for tab in args {
        ctx.world.setup(&["open", "--name", tab, "--size", "20x3", "cat"]);
    }
}

fn open_unnamed(ctx: &mut Ctx<Terminal>, _: &[String], _: Option<&DataTable>) {
    ctx.world.act(&["open", "--name", "cat", "--size", "20x3", "cat"]);
}

fn open_without_a_tab_name(ctx: &mut Ctx<Terminal>, _: &[String], _: Option<&DataTable>) {
    ctx.world.act(&["open", "--size", "20x3", "cat"]);
}

fn rename(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    ctx.world.setup(&["--tab", arg(args, 0), "tab", "rename", arg(args, 1)]);
}

fn snapshot_of(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    ctx.world.act(&["--json", "--tab", arg(args, 0), "snapshot"]);
}

fn error_says(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    let world = &ctx.world;
    let said = world.ran().is_some_and(|ran| ran.stderr.contains(arg(args, 0)));
    assert!(said, "{}", world.report());
}

fn screen_of(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    let world = &ctx.world;
    let tab = world.ran().map(|ran| json(&ran.stdout).get("tab").and_then(Value::as_str).map(String::from));
    assert_eq!(tab, Some(Some(arg(args, 0).to_string())), "{}", world.report());
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
