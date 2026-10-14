# Contributing

## Setup

Turn on the repo's git hooks once per clone:

```sh
git config core.hooksPath .githooks
```

## Rules

- **Conventional Commits.** Every commit's first line is `<type>(<optional scope>): <description>`, with type one of `build chore ci docs feat fix perf refactor revert style test`, and at most 100 characters. The `commit-msg` hook blocks anything else, and CI checks every commit pushed to `main` or proposed in a pull request. Reference the Linear issue in the body, e.g. `Refs: 46H-20`.
- **Names and tests carry the meaning.** Code has no comments (doc comments included) and no workaround markers; `cli/tests/static_checks.rs` lists every banned pattern and `cargo test` fails on any of them.
- **Fix causes, not symptoms.** The judgement calls code review applies are in `CODING_STANDARDS.md`.

## Checks

The `pre-commit` hook runs these whenever a commit touches `cli/`; CI runs them on every push and pull request.

```sh
cd cli
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Dashboard

The dashboard's web app lives in `dashboard/` (Vite, React, Tailwind, xterm.js). Its production build is committed in `cli/assets/dashboard` and embedded into the binary, so `cargo build` and `cargo test` need no Node. After changing anything in `dashboard/`, rebuild and commit the output; CI fails when the committed build does not match the sources.

```sh
cd dashboard
npm ci
npm run build    # type-checks, then writes ../cli/assets/dashboard
npm run dev      # hot reload on :5173, API proxied to a dashboard on :4849
```

The browser tests drive the real dashboard with agent-browser against a real daemon. They are opt-in so contributors without a browser can still run the checks above; CI runs them on every push and pull request.

```sh
npm install --global agent-browser && agent-browser install
cd cli
cargo test --features browser-tests --test cli browser::
```
