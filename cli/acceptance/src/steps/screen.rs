use gherkin_cargo_test::{Ctx, DataTable, StepRegistry};
use serde_json::Value;

use super::WAIT_MS;
use super::common::{self, json};
use super::{Steps, arg};
use crate::programs;
use crate::terminal::Terminal;

pub(crate) fn define(registry: &mut StepRegistry<Terminal>) {
    let mut steps = Steps(registry);
    common::define(&mut steps);
    steps
        .given(r"^a terminal running a program that prints (\d+) lines$", printing)
        .given(r"^a terminal running a program that prints (\d+) lines (\d+) ms apart$", printing_slowly)
        .when(r"^the agent takes a snapshot$", snapshot)
        .when(r#"^the agent waits for "([^"]*)"$"#, wait_for)
        .when(r#"^the agent waits (\d+) ms for "([^"]*)"$"#, wait_briefly_for)
        .then(r#"^the screen's first rows are "([^"]*)", "([^"]*)" and "([^"]*)"$"#, first_rows_are)
        .then(r#"^the screen shows "([^"]*)"$"#, screen_shows)
        .then(r#"^the error names the text it waited for, "([^"]*)"$"#, error_names);
}

fn open_printer(ctx: &mut Ctx<Terminal>, options: &[&str]) {
    match programs::fixture("line-printer") {
        Ok(printer) => {
            let printer = printer.to_string_lossy().into_owned();
            let mut args = vec!["open", "--size", "30x6", printer.as_str(), "--hold"];
            args.extend_from_slice(options);
            ctx.world.setup(&args);
        }
        Err(failure) => ctx.world.failure = Some(failure),
    }
}

fn printing(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    open_printer(ctx, &["--lines", arg(args, 0)]);
    let last = format!("line {}", arg(args, 0));
    ctx.world.setup(&["--timeout", WAIT_MS, "wait", &last]);
}

fn printing_slowly(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    open_printer(ctx, &["--lines", arg(args, 0), "--delay-ms", arg(args, 1)]);
}

fn snapshot(ctx: &mut Ctx<Terminal>, _: &[String], _: Option<&DataTable>) {
    ctx.world.act(&["--json", "snapshot"]);
}

fn wait_for(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    ctx.world.act(&["--timeout", WAIT_MS, "wait", arg(args, 0)]);
}

fn wait_briefly_for(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    ctx.world.act(&["--timeout", arg(args, 0), "wait", arg(args, 1)]);
}

fn lines(text: &str) -> Vec<String> {
    let snapshot = json(text);
    snapshot
        .get("lines")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|line| line.as_str().map(String::from))
        .collect()
}

fn first_rows_are(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    let world = &ctx.world;
    let shown = world.ran().map(|ran| lines(&ran.stdout).into_iter().take(3).collect::<Vec<_>>());
    assert_eq!(shown.as_deref(), Some(args), "{}", world.report());
}

fn screen_shows(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    let snapshot = ctx.world.run(&["--json", "snapshot"]);
    let shown = snapshot.as_ref().map(|ran| lines(&ran.stdout));
    assert!(shown.as_ref().is_ok_and(|shown| shown.iter().any(|line| line == arg(args, 0))), "{shown:?}");
}

fn error_names(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    let world = &ctx.world;
    let quoted = format!("\"{}\"", arg(args, 0));
    assert!(world.ran().is_some_and(|ran| ran.stderr.contains(&quoted)), "{}", world.report());
}
