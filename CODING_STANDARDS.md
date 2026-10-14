# Coding standards

Judgement calls for code review. Mechanical rules live in `cli/tests/static_checks.rs` and the lints in `cli/Cargo.toml`, not here.

## Fix the cause

- **Waits wait on events.** Code that needs something to happen blocks on the signal that says it happened (a condvar, a pipe, a lock, process exit), never on elapsed time. A timeout bounds a wait; it is never the wait.
- **Errors go somewhere.** Every failure is returned to the caller, or logged with what failed and why when there is no caller (daemon threads log to `daemon.log`). `.ok()`, `unwrap_or_default()` and empty `Err(_) =>` arms that turn a failure into a normal-looking value need a reason the reviewer accepts.
- **No special cases for tests.** Product code does not branch on being under test, and a test is never made to pass by bending the code it checks.
- **Races are designed out.** Concurrent paths (several CLIs at once, a program exiting mid-request) are serialized or made idempotent, and covered by a test that runs them together.

## Tests

- **Tests go through the CLI** (`cli/tests/cli`): run the real binary against the fixtures and assert on stdout, stderr, exit codes and files.
- **Tests wait for what they assert.** When a test checks screen content, it opens or acts with `--wait "<text>"` for that content; "stable" is a heuristic and makes tests flaky under load.
- **Expected values are written by hand**, from the spec or a worked example: inline snapshots typed out, not accepted from the tool's output.

## Shape

- **Libraries stay in their module**: the screen engine in `native/screen.rs`, the PTY library in `native/pty.rs`. Other modules use their types.
- **Vocabulary from `GLOSSARY.md`** in names, messages and docs.
