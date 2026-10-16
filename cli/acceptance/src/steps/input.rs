use gherkin_cargo_test::{Ctx, DataTable};
use serde_json::Value;

use super::WAIT_MS;
use super::common::json;
use super::{Steps, arg};
use crate::programs;
use crate::terminal::Terminal;

pub(crate) fn define(steps: &mut Steps<'_>) {
    steps
        .given(r"^a terminal running a program that echoes its input$", echoing)
        .when(r#"^the agent types "([^"]*)"$"#, types)
        .when(r#"^the agent presses "([^"]*)" then "([^"]*)"$"#, presses)
        .when(r#"^the agent sends the bytes "([^"]*)"$"#, sends)
        .then(r#"^the program received "([^"]*)"$"#, received);
}

fn echoing(ctx: &mut Ctx<Terminal>, _: &[String], _: Option<&DataTable>) {
    match programs::fixture("input-echo") {
        Ok(echo) => ctx.world.setup(&["open", "--size", "100x4", "--wait", "ready", &echo.to_string_lossy()]),
        Err(failure) => ctx.world.failure = Some(failure),
    }
}

fn types(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    ctx.world.act(&["type", arg(args, 0)]);
}

fn presses(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    ctx.world.act(&["press", arg(args, 0), arg(args, 1)]);
}

fn sends(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    ctx.world.act(&["send", arg(args, 0)]);
}

fn received(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    let expected = arg(args, 0);
    let waited = ctx.world.run(&["--timeout", WAIT_MS, "wait", expected]).map(|ran| ran.code);
    let snapshot = ctx.world.run(&["--json", "snapshot"]).map(|ran| json(&ran.stdout));
    let second_row =
        snapshot.as_ref().map(|snapshot| snapshot.pointer("/lines/1").and_then(Value::as_str).map(String::from));
    assert_eq!((waited, second_row), (Ok(Some(0)), Ok(Some(expected.to_string()))));
}
