use std::error::Error;
use std::process::{ExitCode, Termination};
use std::sync::mpsc;

use signal_hook::consts::signal::SIGHUP;
use signal_hook::iterator::Signals;

pub type Failure = Box<dyn Error + Send + Sync>;

pub fn until_hang_up<T: Termination + Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    on_hang_up: impl FnOnce() -> T + Send + 'static,
) -> ExitCode {
    let mut hang_ups = match Signals::new([SIGHUP]) {
        Ok(hang_ups) => hang_ups,
        Err(e) => return Err::<(), _>(e).report(),
    };
    let (ended, outcome) = mpsc::channel();
    let hung_up = ended.clone();
    std::thread::spawn(move || hang_ups.forever().next().map(|_| hung_up.send(on_hang_up())));
    std::thread::spawn(move || ended.send(work()));
    outcome.recv().map_or(ExitCode::FAILURE, Termination::report)
}
