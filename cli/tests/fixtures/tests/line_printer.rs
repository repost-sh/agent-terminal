use std::process::Command;

use proptest::test_runner::Config;

proptest::proptest! {
    #![proptest_config(Config { cases: 64, failure_persistence: None, ..Config::default() })]

    #[test]
    fn line_printer_prints_each_numbered_line_then_exits_with_the_code_asked_for(lines in 0u32..40, code in 0u8..=255) {
        let printed = Command::new(env!("CARGO_BIN_EXE_line-printer"))
            .args(["--lines", &lines.to_string(), "--exit", &code.to_string()])
            .output()
            .expect("run line-printer");
        let expected: Vec<String> = (1..=lines).map(|line| format!("line {line}")).collect();
        let stdout = String::from_utf8_lossy(&printed.stdout).into_owned();
        proptest::prop_assert_eq!(stdout.lines().collect::<Vec<_>>(), expected);
        proptest::prop_assert!(printed.stdout.is_empty() || printed.stdout.ends_with(b"\n"));
        proptest::prop_assert_eq!(printed.status.code(), Some(i32::from(code)));
    }
}
