use std::io::{self, Read};
use std::time::Duration;

use super::{Settled, Starting, Terminal};
use crate::native::changes::Changes;
use crate::native::pty;
use crate::native::store::SessionFolder;
use crate::native::wait::Condition;
use crate::protocol::{Size, TextMatch, Wait};

struct InterruptedOnce {
    interrupted: bool,
    reader: Box<dyn Read + Send>,
}

impl Read for InterruptedOnce {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.interrupted {
            return self.reader.read(buf);
        }
        self.interrupted = true;
        Err(io::ErrorKind::Interrupted.into())
    }
}

#[test]
fn a_read_of_the_programs_output_that_a_signal_interrupts_is_retried() {
    let home = tempfile::tempdir().unwrap();
    let command = pty::Command {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), "printf ready; read line".into()],
        cwd: "/".into(),
        env: Vec::new(),
        size: Size { cols: 20, rows: 3 },
    };
    let mut spawned = pty::spawn(&command).unwrap();
    let reader = std::mem::replace(&mut spawned.reader, Box::new(io::empty()));
    spawned.reader = Box::new(InterruptedOnce { interrupted: false, reader });
    let command_log = SessionFolder::start(home.path(), "s").unwrap().command_log("t1");
    let starting =
        Starting { command, shell: None, spawned, command_log, lifecycle: Changes::default(), continued: None };
    let terminal = Terminal::start(starting);
    let ready = Condition::new(&Wait::Text(TextMatch::plain("ready".into()))).unwrap();
    let settled = terminal.wait(ready, Duration::from_secs(10));
    terminal.stop(Duration::from_secs(5));
    assert!(matches!(settled, Settled::Met { .. }), "{:?}", terminal.lines());
}
