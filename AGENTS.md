# agent-terminal

Terminal automation CLI for AI agents, the terminal sibling of agent-browser.

- Rust crate: `cli/` (CLI side in `cli/src`, daemon side in `cli/src/native`, fixture programs in `cli/tests/fixtures`).
- Skills shipped with the binary: `skills/` (stub) and `skill-data/` (guides).
- Spec: Linear 46H-19 and its sub-issues. Command contract: `docs/commands.md`. Design: `docs/design.md`. Vocabulary: `GLOSSARY.md`.
- Dashboard UI reference: the prototype under `.superpowers/prototypes/` (local, gitignored).

Read `CONTRIBUTING.md` before writing code or a commit: no comments in code, no workaround markers, no hacks, Conventional Commits. Enable the hooks once per clone with `git config core.hooksPath .githooks`.

Checks, from `cli/`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.

## Agent skills

### Issue tracker

Linear: team 46h, project agent-terminal; blockers are native "blocked by" links. See `docs/agents/issue-tracker.md`.

### Triage labels

The five default labels (needs-triage, needs-info, ready-for-agent, ready-for-human, wontfix). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
