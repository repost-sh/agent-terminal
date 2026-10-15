use crate::programs::{at_perf, stderr};

const USAGE: &str = "usage: at-perf measure --base <agent-terminal> --change <agent-terminal> --fixtures <dir> --work <empty dir> --out <results.json>\n       at-perf check <results.json> [--summary <file>]\n       at-perf load --agent <agent-terminal> --fixtures <dir> --work <empty dir> --out <rows.json> [--seconds 10] [--rates 1,30,60,120] [--tabs 1,10,20] [--repaints partial,full] [--viewers none,dashboard]";

fn refused(args: &[&str]) -> String {
    let output = at_perf(args);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert_eq!(output.stdout, b"");
    stderr(&output)
}

#[test]
fn without_a_known_command_it_prints_the_usage() {
    assert_eq!(refused(&[]), format!("error: {USAGE}\n"));
    assert_eq!(refused(&["profile"]), format!("error: {USAGE}\n"));
}

#[test]
fn measure_names_the_argument_it_cannot_use() {
    assert_eq!(refused(&["measure", "--fast"]), format!("error: unexpected argument --fast\n{USAGE}\n"));
    assert_eq!(refused(&["measure", "--base"]), "error: --base needs a path\n");
    assert_eq!(refused(&["measure", "--base", "a"]), format!("error: --change is missing\n{USAGE}\n"));
    assert_eq!(
        refused(&["measure", "--base", "a", "--change", "b", "--fixtures", "c", "--work", "d"]),
        format!("error: --out is missing\n{USAGE}\n")
    );
}

#[test]
fn load_names_the_argument_it_cannot_use() {
    let refusals = [
        (&["load", "--fast", "yes"][..], format!("error: unexpected argument --fast\n{USAGE}\n")),
        (&["load", "--seconds"], "error: --seconds needs a value\n".to_string()),
        (
            &["load", "--seconds", "ten"],
            "error: --seconds needs a whole number: invalid digit found in string\n".to_string(),
        ),
        (
            &["load", "--rates", "30,fast"],
            "error: --rates needs whole numbers: invalid digit found in string\n".to_string(),
        ),
        (&["load", "--tabs", "-1"], "error: --tabs needs whole numbers: invalid digit found in string\n".to_string()),
        (
            &["load", "--repaints", "partial,sideways"],
            "error: sideways is not a repaint; use partial or full\n".to_string(),
        ),
        (&["load", "--viewers", "browser"], "error: browser is not a viewer; use none or dashboard\n".to_string()),
        (&["load"], format!("error: --agent is missing\n{USAGE}\n")),
        (&["load", "--agent", "a", "--fixtures", "b", "--work", "c"], format!("error: --out is missing\n{USAGE}\n")),
    ];
    for (args, refusal) in refusals {
        assert_eq!(refused(args), refusal, "{args:?}");
    }
}

#[test]
fn check_names_the_argument_it_cannot_use() {
    assert_eq!(refused(&["check"]), format!("error: {USAGE}\n"));
    assert_eq!(refused(&["check", "a.json", "b.json"]), format!("error: unexpected argument b.json\n{USAGE}\n"));
    assert_eq!(refused(&["check", "a.json", "--summary"]), "error: --summary needs a file\n");
    let missing = refused(&["check", "/nonexistent/results.json"]);
    assert!(missing.starts_with("error: read /nonexistent/results.json: "), "{missing}");
}

#[test]
fn check_refuses_results_that_are_not_json() {
    let dir = tempfile::tempdir().expect("scratch folder");
    let path = dir.path().join("results.json");
    std::fs::write(&path, "not json").expect("write the results");
    let refusal = refused(&["check", path.to_str().expect("a UTF-8 path")]);
    assert!(refusal.starts_with(&format!("error: parse {}: expected ident", path.display())), "{refusal}");
}
