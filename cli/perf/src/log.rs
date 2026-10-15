use std::fmt;

use anyhow::{Result, anyhow};

pub(crate) fn start() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .without_time()
        .with_level(false)
        .with_target(false)
        .try_init()
        .map_err(|e| anyhow!("start the log: {e}"))
}

pub(crate) fn note(happened: fmt::Arguments<'_>) {
    tracing::info!("{happened}");
}
