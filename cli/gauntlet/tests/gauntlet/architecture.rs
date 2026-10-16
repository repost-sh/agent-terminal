use crate::scratch::{Run, Scratch};

const CROSSES: &str = "move what both need into a contract module, or reach it from a layer the policy lets use it → GAUNTLET.md#architecture";

const NO_LAYER: &str = "give the module a layer in gauntlet/src/architecture/policy.rs → GAUNTLET.md#architecture";

const NO_ROLE: &str = "give the crate a role in gauntlet/src/architecture/policy.rs → GAUNTLET.md#architecture";

const ACROSS: &str = "drive agent-terminal through its built binary, or move the shared code into the crate that needs it → GAUNTLET.md#architecture";

const NO_PRODUCT: &str =
    "run the gate in the workspace whose agent-terminal crate the policy describes → GAUNTLET.md#architecture";

const PASSED: &str =
    "every crate depends on another only as its role allows\nagent-terminal: every module keeps to its layer\n";

fn product(files: &[(&str, &str)]) -> Run {
    Scratch::package("agent-terminal", files).architecture()
}

fn product_needing(dependencies: &str) -> Scratch {
    let manifest =
        format!("[package]\nname = \"agent-terminal\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n{dependencies}");
    Scratch::package("agent-terminal", &[("main.rs", "fn main() {}\n")]).with("Cargo.toml", &manifest)
}

fn crossing(file: &str, from: &str, to: &str) -> String {
    format!("agent-terminal/src/{file}: {from} ({}) uses {to} ({}): {CROSSES}\n1 offense(s)\n", layer(from), layer(to))
}

fn layer(module: &str) -> &'static str {
    match module {
        "main" => "entry",
        "commands" | "connection" => "client",
        "native" => "daemon",
        "output" => "presentation",
        _ => "contract",
    }
}

#[test]
fn modules_that_keep_to_their_layers_pass() {
    let run = product(&[
        ("main.rs", "mod commands;\nmod native;\nmod output;\nmod protocol;\n\nfn main() {\n    commands::run();\n}\n"),
        ("commands.rs", "pub fn run() {\n    crate::native::serve();\n    crate::output::show();\n}\n"),
        ("native/mod.rs", "mod daemon;\n\npub fn serve() {\n    daemon::answer();\n}\n"),
        (
            "native/daemon.rs",
            "use crate::output::show;\nuse crate::protocol::Request;\n\npub fn answer() {\n    show();\n    let _ = Request;\n}\n",
        ),
        ("output.rs", "use crate::protocol::Request;\n\npub fn show() {\n    let _ = Request;\n}\n"),
        ("protocol.rs", "pub struct Request;\n"),
    ]);
    assert_eq!((run.code, run.stdout.as_str(), run.stderr.as_str()), (0, PASSED, ""));
}

#[test]
fn each_layer_uses_only_the_layers_it_may() {
    for (from, to, allowed) in [
        ("main", "commands", true),
        ("main", "protocol", true),
        ("main", "native", false),
        ("main", "output", false),
        ("commands", "connection", true),
        ("commands", "native", true),
        ("commands", "output", true),
        ("commands", "protocol", true),
        ("native", "output", true),
        ("native", "protocol", true),
        ("native", "commands", false),
        ("output", "protocol", true),
        ("output", "native", false),
        ("output", "commands", false),
        ("protocol", "key", true),
        ("protocol", "output", false),
        ("protocol", "native", false),
        ("protocol", "commands", false),
    ] {
        let to_file = format!("{to}.rs");
        let from_file = format!("{from}.rs");
        let run = if from == "main" {
            product(&[
                ("main.rs", &format!("mod {to};\n\nfn main() {{\n    {to}::f();\n}}\n")),
                (&to_file, "pub fn f() {}\n"),
            ])
        } else {
            product(&[
                ("main.rs", &format!("mod {to};\nmod {from};\n\nfn main() {{}}\n")),
                (&from_file, &format!("pub fn g() {{\n    crate::{to}::f();\n}}\n")),
                (&to_file, "pub fn f() {}\n"),
            ])
        };
        let expected = if allowed { PASSED.to_string() } else { crossing(&from_file, from, to) };
        assert_eq!((run.code, run.stdout), (if allowed { 0 } else { 2 }, expected), "{from} uses {to}");
    }
}

#[test]
fn every_way_the_daemon_can_name_a_client_module_is_caught() {
    let main = "mod connection;\nmod native;\n\nfn main() {}\n";
    let connection = "pub fn home() {}\npub const NAME: &str = \"n\";\npub struct Launch;\n";
    let tokens = "macro_rules! m {\n    ($($t:tt)*) => {};\n}\n";
    for (file, code) in [
        ("native.rs", "use crate::connection::home;\n".to_string()),
        ("native.rs", "use crate::{connection::{NAME, home}};\n".to_string()),
        ("native.rs", "use crate::connection::home as at_home;\n".to_string()),
        ("native.rs", "use crate::connection::*;\n".to_string()),
        ("native.rs", "pub fn f() {\n    crate::connection::home();\n}\n".to_string()),
        ("native.rs", "pub fn f(_: crate::connection::Launch) {}\n".to_string()),
        ("native.rs", "pub fn f() -> String {\n    format!(\"{}\", crate::connection::NAME)\n}\n".to_string()),
        ("native.rs", format!("{tokens}m!(take crate::connection::NAME);\n")),
        ("native.rs", format!("{tokens}m!(take [crate::connection::NAME]);\n")),
        ("native.rs", "pub fn f() {\n    super::connection::home();\n}\n".to_string()),
        ("native.rs", "mod tests {\n    use super::super::connection::home;\n}\n".to_string()),
        ("native/deep/inner.rs", "use super::super::super::connection::home;\n".to_string()),
    ] {
        let run = product(&[("main.rs", main), ("connection.rs", connection), (file, &code)]);
        assert_eq!((run.code, run.stdout), (2, crossing(file, "native", "connection")), "{code}");
    }
}

#[test]
fn names_that_only_look_like_a_client_module_pass() {
    let native = "const SAID: &str = \"crate::connection::home\";

pub(crate) fn f() {}

pub(super) fn g() {}

mod connection {
    pub fn home() {}
}

mod tests {
    use super::connection::home;
}

pub fn h() {
    connection::home();
    self::connection::home();
}

macro_rules! m {
    () => {
        pub(crate) fn i() {}
    };
}
";
    let run = product(&[
        ("main.rs", "mod connection;\nmod native;\n\nfn main() {}\n"),
        ("connection.rs", "pub fn home() {}\n"),
        ("native.rs", native),
    ]);
    assert_eq!((run.code, run.stdout.as_str()), (0, PASSED));
}

#[test]
fn the_crate_root_is_the_entry_and_no_module_uses_it() {
    let main = "mod commands;\nmod protocol;\n\npub const VERSION: &str = \"1\";\n\nfn main() {}\n";
    for (file, user, code) in [
        ("protocol.rs", "protocol", "pub fn v() -> &'static str {\n    crate::VERSION\n}\n"),
        ("protocol.rs", "protocol", "pub fn v() -> &'static str {\n    super::VERSION\n}\n"),
        ("protocol.rs", "protocol", "use super::*;\n"),
        ("commands.rs", "commands", "pub fn v() -> &'static str {\n    crate::VERSION\n}\n"),
    ] {
        let run = product(&[("main.rs", main), (file, code)]);
        assert_eq!((run.code, run.stdout), (2, crossing(file, user, "main")), "{code}");
    }
}

#[test]
fn a_module_without_a_layer_fails_once() {
    let run = product(&[
        ("main.rs", "mod gadget;\nmod native;\n\nfn main() {}\n"),
        ("gadget/mod.rs", "mod inner;\n\npub fn f() {\n    crate::native::g();\n}\n"),
        ("gadget/inner.rs", "pub fn i() {}\n"),
        ("native.rs", "pub fn g() {\n    crate::gadget::f();\n}\n"),
    ]);
    let expected = format!("agent-terminal/src/gadget/mod.rs: gadget has no layer: {NO_LAYER}\n1 offense(s)\n");
    assert_eq!((run.code, run.stdout), (2, expected));
}

#[test]
fn a_module_that_does_not_parse_is_an_error_naming_it() {
    let run = product(&[("main.rs", "mod native;\n\nfn main() {}\n"), ("native.rs", "fn (\n")]);
    assert!(run.stderr.starts_with("error: parse "), "{}", run.stderr);
    assert!(run.stderr.contains("agent-terminal/src/native.rs: "), "{}", run.stderr);
    assert_eq!(run.code, 1);
}

#[test]
fn the_product_takes_black_box_test_crates_as_dev_dependencies_only() {
    let run = product_needing("[dev-dependencies]\nat-acceptance = { path = \"../at-acceptance\" }\n")
        .member("at-acceptance", "")
        .member("at-perf", "[dependencies]\nserde = \"1\"\n")
        .architecture();
    assert_eq!((run.code, run.stdout.as_str(), run.stderr.as_str()), (0, PASSED, ""));
}

#[test]
fn crates_that_depend_across_their_roles_fail() {
    let run = product_needing(
        "[dependencies]\nat-perf = { path = \"../at-perf\" }\n\n[dev-dependencies]\nat-gauntlet = { path = \"../at-gauntlet\" }\n",
    )
    .member("at-acceptance", "")
    .member("at-perf", "")
    .member("at-gauntlet", "")
    .member(
        "at-fixtures",
        "[dev-dependencies]\nat-perf = { path = \"../at-perf\" }\n\n[build-dependencies]\nat-acceptance = { path = \"../at-acceptance\" }\n",
    )
    .architecture();
    let expected = [
        "agent-terminal (product): a normal dependency on at-perf (black-box tests)",
        "agent-terminal (product): a dev dependency on at-gauntlet (tooling)",
        "at-fixtures (black-box tests): a dev dependency on at-perf (black-box tests)",
        "at-fixtures (black-box tests): a build dependency on at-acceptance (black-box tests)",
    ]
    .map(|offense| format!("{offense}: {ACROSS}\n"))
    .concat();
    assert_eq!((run.code, run.stdout), (2, format!("{expected}4 offense(s)\n")));
}

#[test]
fn a_crate_without_a_role_fails() {
    let run = Scratch::package("agent-terminal", &[("main.rs", "fn main() {}\n")]).member("app", "").architecture();
    assert_eq!((run.code, run.stdout), (2, format!("app: the crate has no role: {NO_ROLE}\n1 offense(s)\n")));
}

#[test]
fn a_workspace_without_agent_terminal_fails() {
    let run = Scratch::crate_with(&[("main.rs", "fn main() {}\n")]).architecture();
    let expected = format!(
        "app: the crate has no role: {NO_ROLE}\nthe workspace has no agent-terminal crate: {NO_PRODUCT}\n2 offense(s)\n"
    );
    assert_eq!((run.code, run.stdout), (2, expected));
}
