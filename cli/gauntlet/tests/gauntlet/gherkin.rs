use crate::scratch::{Scratch, run};

const FIX: &str = "bind each Then step through then(…) to a step that asserts, keep features/ir.json what `at-gauntlet gherkin-ir` writes, and mark nothing wip → GAUNTLET.md#acceptance";

const FEATURE: &str = "Feature: Counting
  Scenario: adding one
    Given a count of 1
    When one is added
    Then the count is 2
    And the count is even
";

const STEPS: &str = r#"
pub fn define(steps: &mut Steps) {
    steps.given(r"^a count of (\d+)$", start).when(r"^one is added$", add).then(r"^the count is (\d+)$", is).then(r"^the count is even$", even);
}
fn start(ctx: &mut Ctx, args: &[String]) { ctx.count = 1; }
fn add(ctx: &mut Ctx, _: &[String]) { ctx.count += 1; }
fn is(ctx: &mut Ctx, args: &[String]) { assert_eq!(ctx.count.to_string(), args[0]); }
fn even(ctx: &mut Ctx, _: &[String]) { assert!(ctx.count % 2 == 0); }
"#;

fn suite(steps: &str) -> Scratch {
    let scratch = Scratch::crate_with(&[("lib.rs", steps)]).with("features/counting.feature", FEATURE);
    assert_eq!(run(&scratch.root(), &["gherkin-ir"]).stdout, "wrote app/features/ir.json\n");
    scratch
}

fn offenses(lines: &[&str]) -> String {
    let count = format!("{} offense(s)", lines.len());
    lines.iter().map(|line| [*line, ": ", FIX, "\n"].concat()).chain([count, "\n".to_string()]).collect()
}

#[test]
fn a_suite_whose_then_steps_all_assert_passes() {
    let checked = run(&suite(STEPS).root(), &["gherkin"]);
    assert_eq!(
        (checked.code, checked.stdout.as_str()),
        (0, "app: every Then step asserts and features/ir.json is current\n")
    );
}

#[test]
fn a_then_step_that_asserts_nothing_or_is_bound_by_given_is_an_offense() {
    let quiet = STEPS.replace("assert!(ctx.count % 2 == 0);", "let _ = ctx.count;");
    let checked = run(&suite(&quiet).root(), &["gherkin"]);
    assert_eq!(
        (checked.code, checked.stdout),
        (
            2,
            offenses(&[
                r#"app/features/counting.feature:6: Then "the count is even" runs even, which asserts nothing"#
            ])
        )
    );
    let given = STEPS.replace(r#".then(r"^the count is even$", even)"#, r#".given(r"^the count is even$", even)"#);
    let checked = run(&suite(&given).root(), &["gherkin"]);
    assert_eq!(
        checked.stdout,
        offenses(&[r#"app/features/counting.feature:6: Then "the count is even" is bound by no then(…) step"#])
    );
}

#[test]
fn a_feature_changed_without_its_ir_or_steps_marked_wip_is_an_offense() {
    let scratch = suite(STEPS);
    std::fs::write(scratch.path("features/counting.feature"), FEATURE.replace("adding one", "adding a one"))
        .expect("edit the feature");
    let checked = run(&scratch.root(), &["gherkin"]);
    assert_eq!(checked.stdout, offenses(&["app/features/ir.json does not match the feature files"]));
    let wip = format!("{STEPS}\nfn later(features: Features) {{ features.wip(\"counting\"); }}\n");
    let checked = run(&suite(&wip).root(), &["gherkin"]);
    assert_eq!(checked.stdout, offenses(&["app: the steps mark a feature or scenario wip"]));
}

#[test]
fn a_workspace_without_acceptance_features_is_an_offense() {
    let checked = run(&Scratch::crate_with(&[("main.rs", "fn main() {}\n")]).root(), &["gherkin"]);
    assert_eq!((checked.code, checked.stdout), (2, offenses(&["the workspace has no crate with acceptance features"])));
}
