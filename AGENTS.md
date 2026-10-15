# agent-terminal

Terminal automation CLI for AI agents, the terminal sibling of agent-browser.

- Rust crate: `cli/` (CLI side in `cli/src`, daemon side in `cli/src/native`, fixture programs in `cli/tests/fixtures`).
- Skills shipped with the binary: `skills/` (stub) and `skill-data/` (guides).
- Packaging and releases: npm launcher in `npm/`, dist config in `dist-workspace.toml`, changelog in `CHANGELOG.md`.
- Spec: Linear 46H-19 and its sub-issues.
- Dashboard: web app in `dashboard/` (Vite, React, Tailwind, xterm.js), built into `cli/assets/dashboard` (committed, embedded in the binary); server in `cli/src/native/dashboard`.

Before writing code or a commit: no comments in code, no workaround markers, no hacks, Conventional Commits. Enable the hooks once per clone with `git config core.hooksPath .githooks`.

Checks, from `cli/`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. From the repo root, when `npm/` changes: `node --test 'npm/test/*.test.js'`. When `dashboard/` changes: `npm ci`, `npm run lint`, `npm run build` in `dashboard/`, commit `cli/assets/dashboard`, and run the browser tests from `cli/` (needs agent-browser): `cargo test --features browser-tests --test cli browser::`.

## Agent skills

### Issue tracker

Linear: team 46h, project agent-terminal; blockers are native "blocked by" links.

### Triage labels

The five default labels (needs-triage, needs-info, ready-for-agent, ready-for-human, wontfix).
