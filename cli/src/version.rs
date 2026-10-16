pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) fn this_version() -> Result<semver::Version, semver::Error> {
    semver::Version::parse(VERSION)
}
