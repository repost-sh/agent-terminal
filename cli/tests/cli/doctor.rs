use crate::support::AgentTerminal;

#[test]
fn doctor_is_an_unknown_command() {
    let at = AgentTerminal::without_session();
    for args in [&["doctor"][..], &["doctor", "--fix"]] {
        let run = at.run(args);
        assert_eq!((run.code(), run.stdout().as_str()), (1, ""), "agent-terminal {}", args.join(" "));
        assert!(run.stderr().contains("unrecognized subcommand 'doctor'"), "{}", run.stderr());
    }
}
