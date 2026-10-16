pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) const COMMIT: &str = env!("AGENT_TERMINAL_COMMIT");

pub(crate) const BUILD: &str = concat!(env!("CARGO_PKG_VERSION"), "+", env!("AGENT_TERMINAL_COMMIT"));

pub(crate) fn this_version() -> Result<semver::Version, semver::Error> {
    semver::Version::parse(VERSION)
}

pub(crate) fn daemon_of(build: Option<&str>) -> String {
    match build {
        Some(build) => format!("agent-terminal {build}"),
        None => "an agent-terminal that does not record its build".to_string(),
    }
}
