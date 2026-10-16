use super::masked;

#[test]
fn the_environment_is_sorted_and_a_secret_named_variable_shows_no_value() {
    let env = [
        "PATH=/bin",
        "API_TOKEN=t",
        "db_password=p",
        "aws_secret=s",
        "SSH_KEY=k",
        "PASSWD=w",
        "GIT_CREDENTIAL=c",
        "HOME=/h",
    ]
    .map(|pair| pair.split_once('=').map(|(name, value)| (name.to_string(), value.to_string())).unwrap());
    let shown: Vec<String> = masked(env.to_vec())
        .into_iter()
        .map(|var| format!("{}={}", var.name, var.value.unwrap_or_else(|| "<hidden>".to_string())))
        .collect();
    assert_eq!(
        shown,
        [
            "API_TOKEN=<hidden>",
            "GIT_CREDENTIAL=<hidden>",
            "HOME=/h",
            "PASSWD=<hidden>",
            "PATH=/bin",
            "SSH_KEY=<hidden>",
            "aws_secret=<hidden>",
            "db_password=<hidden>"
        ]
    );
}
