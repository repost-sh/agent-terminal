# agent-terminal

Terminal automation CLI for AI agents, the terminal sibling of agent-browser. Specs and issues live in Linear: team 46H, project agent-terminal, blockers as native "blocked by" links.

## Map

- `cli/` is the Rust crate. A command runs `cli/src/commands.rs` (clap args, built into a `Request`) → `cli/src/protocol.rs` (the wire types between CLI and daemon) → `cli/src/native/daemon.rs` (handles the `Request`) → `cli/src/output.rs` (prints the `Reply`). Everything under `cli/src/native/` runs in the daemon.
- `cli/tests/cli/` holds one file per command. Each test runs the real binary through the harness in `support.rs` against the fixture programs in `cli/tests/fixtures/src/bin/`.
- `dashboard/` is the web app (Vite, React, Tailwind, xterm.js), served by `cli/src/native/dashboard/`. Its wire types in `dashboard/src/lib/protocol.ts` are written by hand to match `cli/src/protocol.rs`, so change both together. `npm run build` writes the app into `cli/assets/dashboard`, which is committed and embedded in the binary.
- `skill-data/` holds the guides `agent-terminal skills get` prints, compiled into the binary by `cli/src/skills.rs`. When a command's flags or behaviour change, the guide that teaches it changes in the same commit. `skills/` holds the discovery stub.
- Releases: `npm/` (launcher and per-platform packages), `dist-workspace.toml`, `.github/workflows/release.yml`.

## Checks

`scripts/gauntlet.sh` is the gauntlet, the one entry point for every check: work is done when it ends `GAUNTLET GREEN`. Its areas are `rust` (`cli/`), `dashboard`, `npm` and `browser`; with no argument it runs `rust dashboard npm`. Each failure names its fix.

Rule Zero: fix the code a check flags, and leave every rule as it stands; a rule that seems wrong goes to the owner.

`.githooks/pre-commit` runs the areas a commit touches. Enable it once per clone with `git config core.hooksPath .githooks`.

`scripts/gauntlet.sh browser` runs the dashboard's browser tests; they need `agent-browser` on `PATH`, with `AGENT_BROWSER_EXECUTABLE_PATH` pointing at Chrome when agent-browser does not find one itself.

## Code Locality

- Keep logic in the module that owns the behaviour. `daemon.rs` owns sessions, tabs and dispatch; what happens inside one tab belongs in `terminal.rs` or a module beside it (`wait.rs`, `refs.rs`, `screenshots.rs`, `store.rs` for the session folder).
- Logic for one command goes in that command's module, not in `commands.rs`, `protocol.rs` or `daemon.rs`, which every command passes through.
- Each library sits behind the one module that wraps it, as `cli/tests/static_checks.rs` enforces for `alacritty_terminal` (`native/screen.rs`) and `portable_pty` (`native/pty.rs`). Give a new library the same treatment; other modules use the wrapper's types.
- Code carries no comments: names and tests say what it does. `static_checks.rs` fails `cargo test` on a comment in Rust and on each workaround it lists; fix the cause rather than route around the check.

## No Junk Drawers

A file holds one concern and is named for it. If you cannot describe a file in one sentence without "and", it is a junk drawer.

- Name modules for what they own (`output_log.rs`, `session_id.rs`, `screenshot-lightbox.tsx`), not for where they sit or what kind of code they hold. `support`, `utils`, `helpers`, `common`, `misc` and `shared` describe a location, not a concern; a file with one of those names still owns a single concern scoped to its folder, as `cli/tests/cli/support.rs` owns the harness every CLI test drives.
- Before adding code to an existing file, check that it shares the file's concern. Being imported by the same callers, or living in the same folder, does not make it the same concern. Put it in the module that owns the concern, or in a new file named for it.
- Put code next to the contract it serves: a type that crosses the socket in `protocol.rs`, how a reply reads as text in `output.rs`.
- When an area covers several concerns, give it a folder with one file per concern and a `mod.rs` for the entry point they share. Examples: `cli/src/native/render/` (boxes / diff / fonts / theme / vector), `cli/src/native/dashboard/`, one command per file in `cli/tests/cli/`, one concern per file in `dashboard/src/lib/`.
- Add new code in its proper home even when a drawer is closer. Splitting a drawer is a separate change, so mention it rather than folding it into unrelated work.

## Abstractions

- Add an abstraction only when it reduces real complexity, encodes a domain rule, or is reused by multiple existing callers.
- Do not add tiny helpers that only rename a simple library call, hide one expression, or save a trivial amount of typing. Repetition of a simple expression is better than a helper that becomes another source of drift.
- Avoid cleanup abstractions that create a second thing developers must discover, remember, and keep consistent.

## Commits

[Conventional Commits](https://www.conventionalcommits.org/). `.githooks/commit-msg`, also run in CI on every commit, enforces the types and the 100-character subject.

- Scope is optional and lowercase: the area (`cli`, `dashboard`, `skills`, `release`, `npm`) or the command (`wait`, `doctor`).
- Summary in imperative mood, lowercase, no trailing period.
- Footer `Refs: 46H-<n>` naming the Linear issue. A branch lands as a merge commit titled `chore: merge 46H-<n>, <what it delivers>`.
- Breaking changes: add `!` after the type/scope and a `BREAKING CHANGE:` footer.
- Commits and PRs carry no attribution: no `Co-Authored-By`, `Claude-Session` or "Generated with" lines, whatever a tool's defaults say.
