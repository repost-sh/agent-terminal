# Contributing

## Setup

Turn on the repo's git hooks once per clone:

```sh
git config core.hooksPath .githooks
```

## Rules

- **Conventional Commits.** Every commit's first line is `<type>(<optional scope>): <description>`, with type one of `build chore ci docs feat fix perf refactor revert style test`, and at most 100 characters. The `commit-msg` hook blocks anything else, and CI checks every commit in a pull request. Reference the Linear issue in the body, e.g. `Refs: 46H-20`.
- **No comments in code**, doc comments included, and **no workaround markers** (`unsafe`, `#[allow]`, `#[expect]`, `#[ignore]`, `todo!`, `unimplemented!`, `dbg!`). `cargo test` fails on any of them (`cli/tests/static_checks.rs`). Names and tests carry the meaning instead.
- **No hacks.** Fix the cause rather than working around it: no sleeps standing in for a real wait, no swallowed errors, no special cases to make a test pass. Code review checks this.

## Checks

```sh
cd cli
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
