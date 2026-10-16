use proptest::test_runner::Config;

const STEPS: [(&str, &str); 7] = [
    ("input", "the agent types \"{}\""),
    ("input", "the program received \"{}\""),
    ("input", "the agent sends the bytes \"{}\""),
    ("screen", "the agent waits for \"{}\""),
    ("screen", "the screen shows \"{}\""),
    ("sessions", "the agent closes the tab in \"{}\""),
    ("sessions", "the list names exactly the sessions \"{}\""),
];

proptest::proptest! {
    #![proptest_config(Config { cases: 128, failure_persistence: None, ..Config::default() })]

    #[test]
    fn any_quoted_argument_binds_exactly_one_step_and_reaches_it_whole(text in "[^\"\\n]{0,24}") {
        for (feature, template) in STEPS {
            let step = template.replace("{}", &text);
            proptest::prop_assert_eq!(at_acceptance::bindings(feature, &step), (1, Some(vec![text.clone()])), "{}", step);
        }
    }
}
