use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let trials = at_acceptance::trials(Path::new(env!("CARGO_BIN_EXE_agent-terminal")));
    libtest_mimic::run(&libtest_mimic::Arguments::from_args(), trials).exit_code()
}
