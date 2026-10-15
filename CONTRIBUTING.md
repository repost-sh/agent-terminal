# Contributing

## Setup

Turn on the repo's git hooks once per clone:

```sh
git config core.hooksPath .githooks
```

## Rules

- **Conventional Commits.** Every commit's first line is `<type>(<optional scope>): <description>`, with type one of `build chore ci docs feat fix perf refactor revert style test`, and at most 100 characters. The `commit-msg` hook blocks anything else, and CI checks every commit pushed to `main` or proposed in a pull request. Reference the Linear issue in the body, e.g. `Refs: 46H-20`.
- **Names and tests carry the meaning.** Code has no comments (doc comments included) and no workaround markers; `cli/tests/static_checks.rs` lists every banned pattern and `cargo test` fails on any of them.
- **No effect hooks in the dashboard.** `useEffect`, `useLayoutEffect` and `useInsertionEffect`, aliased or namespaced, are banned because effects resynchronise after render, which costs extra renders and breeds races and stale state: subscribe with `useSyncExternalStore`, manage DOM and xterm.js in a ref callback with cleanup, act in event handlers, derive values during render, reset state with `key`; oxlint (`npm run lint` in `dashboard/`, rules in `dashboard/.oxlintrc.json`) fails on any of them.
- **Fix causes, not symptoms.** The judgement calls code review applies are in `CODING_STANDARDS.md`.

## Checks

The `pre-commit` hook runs the first group whenever a commit touches `cli/` and the second whenever it touches `dashboard/`; CI runs both on every push and pull request.

```sh
cd cli
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

```sh
cd dashboard
npm ci
npm run lint     # oxlint, warnings fail it
```

## Speed and memory

CI's `performance` job (macOS and Linux) builds agent-terminal at the base of the change and at the change, measures both release binaries from outside on the same runner with `at-perf` (`cli/perf`), keeps the samples as a run artifact, and fails when a metric is over its limit or regresses against the base. To run it yourself:

```sh
cd cli
cargo build --release --workspace --bins
target/release/at-perf measure --base <agent-terminal built from main> --change target/release/agent-terminal \
  --fixtures target/release --work <new folder> --out results.json
target/release/at-perf check results.json   # exits 2 on a metric over its limit or a regression
```

- Speed: each call is timed from spawning the CLI to its exit, 100 times, base and change alternating, against a tab showing a 120x40 screen of colored words (fixture `busy-screen`). `wait <text>` is timed from the moment the program prints the text to the exit of the `press --wait` call that waits for it.
- Memory: the daemon's resident size from `ps`. Idle: a fresh daemon that has answered one call and has no tabs. Per active tab: what ten tabs add, each a 120x40 screen with its 2,000-line scrollback full. Median of five fresh daemons per side.
- Limits: CLI round trip ≤ 5 ms, `snapshot` ≤ 10 ms and `screenshot` PNG ≤ 40 ms (the speed targets in `docs/design.md`), idle memory ≤ 10 MB. A median over one fails the job.
- Reference: memory per active tab has a reference value of about 5 MB, not a limit. The table shows it and marks a median above it `over reference`, which does not fail the job; with its 2,000-line scrollback full a tab measures about 9 MB, most of it the screen grid. Like every metric it still fails the job when it regresses against the base.
- Limits and regressions are checked against the medians. A regression is a median more than 25% and more than 0.25 ms over the base (memory: 10% and 0.25 MB). Measured against itself, one binary differs by under 3% on an idle machine and by up to 19% with the CPU five times oversubscribed, and memory by under 1%.

## Dashboard

The dashboard's web app lives in `dashboard/` (Vite, React, Tailwind, xterm.js). Its production build is committed in `cli/assets/dashboard` and embedded into the binary, so `cargo build` and `cargo test` need no Node. After changing anything in `dashboard/`, rebuild and commit the output; CI fails when the committed build does not match the sources.

```sh
cd dashboard
npm ci
npm run build    # type-checks, then writes ../cli/assets/dashboard
npm run dev      # hot reload on :5173, API proxied to a dashboard on :4849
```

The dashboard answers only its own pages, so start the one the dev server talks to with the dev server's origin allowed: `agent-terminal dashboard start --allow-origin http://localhost:5173`.

The browser tests drive the real dashboard with agent-browser against a real daemon. They are opt-in so contributors without a browser can still run the checks above; CI runs them on every push and pull request.

```sh
npm install --global agent-browser && agent-browser install
cd cli
cargo test --features browser-tests --test cli browser::
```
