use gherkin_cargo_test::{Ctx, DataTable};
use serde_json::Value;

use super::{Steps, arg};
use crate::terminal::Terminal;

pub(crate) fn define(steps: &mut Steps<'_>) {
    steps
        .then(r"^the command succeeds$", the_command_succeeds)
        .then(r"^the command fails with exit code (\d+)$", the_command_fails_with);
}

pub(crate) fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

fn the_command_succeeds(ctx: &mut Ctx<Terminal>, _: &[String], _: Option<&DataTable>) {
    let world = &ctx.world;
    assert_eq!(world.ran().map(|ran| ran.code), Some(Some(0)), "{}", world.report());
}

fn the_command_fails_with(ctx: &mut Ctx<Terminal>, args: &[String], _: Option<&DataTable>) {
    let world = &ctx.world;
    let code = world.ran().and_then(|ran| ran.code).map(|code| code.to_string());
    assert_eq!(code.as_deref(), Some(arg(args, 0)), "{}", world.report());
}
