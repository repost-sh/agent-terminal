# The gauntlet

Every check agent-terminal holds its code to, behind one entry point: `scripts/gauntlet.sh`. Work is done when it ends `GAUNTLET GREEN`. Each offense line ends with its fix and `→ GAUNTLET.md#<rule>`, the section below that explains the rule.

## Rule Zero

Fix the code a check flags. Never disable, suppress, scope away or weaken a rule to get green: no `#[allow]` or `#[expect]`, no inline ESLint config, no per-file override that turns a rule off, no raised threshold, no allowlist, no config file that changes a tool's options. A rule that seems wrong for this codebase goes to the owner with evidence, never into the same change as the code it flags.

The escape hatches are sealed by checks of their own: [sealed lint table](#sealed-lint-table), [sealed toolchain](#sealed-toolchain), [sealed ESLint config](#sealed-eslint-config), [banned tokens](#banned-tokens) (silenced lints, skipped tests, code hidden from coverage or mutation) and [no comments](#no-comments), so no comment can carry a lint, jscpd or Stryker directive. The duplication and mutation gates refuse any config file their tools would read.

## Areas

| Area | Runs | When |
|---|---|---|
| `rust` | [rust format](#rust-format), [rust lint](#rust-lint) with and without `browser-tests`, [dependency policy](#dependency-policy), [unused dependencies](#unused-dependencies), [rust tests](#rust-tests) (with the static checks and the acceptance suite), [property tests](#property-tests), [architecture](#architecture), [parameter ceiling](#parameter-ceiling), [acceptance](#acceptance) with the names of its faults (needs `cargo-mutants`) | default; pre-commit for `cli/`, `scripts/`, `.githooks/` |
| `dashboard` | [dashboard lint](#dashboard-lint), [dashboard types](#dashboard-types), [gauntlet tools](#gauntlet-tools), [dashboard tests](#dashboard-tests), [CRAP](#crap), [property tests](#property-tests), [dashboard build](#dashboard-build) | default; pre-commit for `dashboard/` |
| `npm` | [npm tests](#npm-tests) | default; pre-commit for `npm/` |
| `duplication` | [duplication](#duplication) | default; pre-commit for `cli/`, `dashboard/`, `scripts/` |
| `browser` | [browser tests](#browser-tests) | needs `agent-browser` and Chrome |
| `coverage` | [coverage floor](#coverage-floor) and [CRAP](#crap) for the Rust crates | needs `cargo-llvm-cov` and `agent-browser` |
| `mutation` | [mutation](#mutation): cargo-mutants and Stryker on the lines the change touched since `main` | each pull request; minutes for a typical change |
| `acceptance-faults` | the fault self-test of [acceptance](#acceptance) | heavy: nightly and before a release |

With no area, `scripts/gauntlet.sh` runs `rust dashboard npm duplication`. Enable the pre-commit hook once per clone with `git config core.hooksPath .githooks`. `git commit --no-verify` skips the hook, so CI runs the gauntlet on every push.

## Rules

### Rust format

`cargo fmt --all --check` in `cli/`. On offense: run `cargo fmt --all` in `cli/`.

### Rust lint

`cargo clippy --workspace --all-targets -- -D warnings`, and again with `--features browser-tests`, against the workspace lint table in `cli/Cargo.toml` and the thresholds in the `clippy.toml` files. On offense: fix the code each finding names.

### Sealed lint table

A static check (`cli/tests/static_checks/lint_table.rs`): the workspace lint table keeps every required lint at its level, every crate takes the workspace lints, and each `clippy.toml` keeps its thresholds, its test exceptions and its banned methods with reasons. On offense: restore the rule and fix the code it flags.

### Sealed toolchain

A static check (`toolchain.rs`): no `RUSTFLAGS`, `cap-lints`, `-A` flag, `CLIPPY_CONF_DIR` or `--no-verify` in build configuration, scripts or test roots, so lints are set only where the lint table says. Test code gets the lint allowances in `clippy.toml`, so `#[cfg(test)]` may mark only a test root's `mod` lines and, in a crate's `src/`, a module's `mod tests;`: its unit tests, in the `tests.rs` beside its `mod.rs`. On offense: remove the flag and fix the code it hid.

### Banned tokens

A static check (`banned_tokens.rs`) over every Rust file: no `unsafe`, `todo!`, `unimplemented!`, `dbg!`, `#[allow(`, `#[expect(`, `#[ignore`, `coverage(off)`, `cfg(coverage`, or `mutants::skip`; in product code no `thread::sleep` and no `let _ =`; and each library only in its home module (`alacritty_terminal` in `native/screen/`, `portable_pty` in `native/pty.rs`, `num_traits` in `native/render/`, `tracing` in the log modules, `toml` in the lint-table check, `syn` and `proc_macro2` in `gauntlet/src/rust/`). On offense: handle the error, wait on the event, move the call into its home module, or write the missing code.

### No comments

Rust (`comments.rs`) and the dashboard (`local/no-comments`) carry no comments: names, types and tests say what code does, and a reason belongs in the commit message. On offense: say it with a name, a type or a test.

### File size

No Rust, TypeScript or JavaScript file over 500 lines (`file_size.rs`). On offense: split the file by concern into a folder with one file per concern (AGENTS.md, No Junk Drawers).

### Artifact paths

Code and tests never name a gitignored scratch folder (`artifact_paths.rs` in Rust, `local/no-ignored-artifact-paths` in the dashboard). On offense: use a temporary folder the test creates.

### Dependency policy

`cargo deny check` against `cli/deny.toml`: licenses, banned crates, sources and advisories. On offense: replace or update the crate it names.

### Unused dependencies

`cargo machete` over every crate. On offense: remove the dependency from its `Cargo.toml`.

### Rust tests

`cargo test --workspace`: the CLI tests drive the built binary, the static checks read the tree, the acceptance suite runs its features, and each tool crate tests itself. On offense: fix the code the failing test or static check names.

### Property tests

Every crate has at least one real property test (`at-gauntlet props`): a `proptest!` with a `#[test]`, an input strategy and an assertion, not switched off by `cfg`. The dashboard has at least one `fc.assert(fc.property(…))` whose predicate asserts (`gauntlet/props.ts`). On offense: state an invariant of the code's behaviour as a property.

### Architecture

Dependencies point one way. `at-gauntlet architecture` resolves every crate path in `cli/src` (use trees, expression and type paths, paths inside macros, `super` chains, inline modules) and holds each module to its layer in `cli/gauntlet/src/architecture/policy.rs`: the contract (`protocol`, `home`, `global_options`, `version`, `exit`, `failure`, `key`, `paint`, `session_id`) uses only the contract; presentation (`output`) uses the contract; the daemon (`native`) uses itself, presentation and the contract; the client uses everything but the crate root; nothing uses the crate root. Crates have roles: only the product takes a black-box test crate, as a dev-dependency. In the dashboard, `local/layers` keeps `src/lib` below `src/components` below `app.tsx` and `main.tsx`; tests import anything. A module, crate or file without a layer fails. On offense: move what both sides need into the lower layer, or hand it down from above.

### Parameter ceiling

A function, method or trait method visible beyond its module takes at most two parameters besides `self` (`at-gauntlet params`, after repost-go's G6). The dashboard holds every function to two (`better-max-params`). On offense: group the parameters into one value with named fields, or keep the function inside its module.

### Trivial helpers

Report only, in no area yet: `at-gauntlet helpers` names each private function with one call site in its crate whose body is one expression with no `if`, `match`, loop or `?`. AGENTS.md's Abstractions rule says a helper that only renames an expression is another place to keep in step. On offense: write the expression at its call site, or keep the helper for a second caller.

### Parameter bags

Report only, in no area yet: `at-gauntlet bags` names each struct with named fields, not serialized, that one struct literal in its crate builds and one function takes, its own methods counted as takers. Such a struct often exists only to get under the [parameter ceiling](#parameter-ceiling). On offense: pass what the function needs itself, or give the struct the methods that use its fields.

### Duplication

No block of 100 tokens appears twice (jscpd 5.4.1 at repost-go's dupl threshold, no line minimum) across every crate's `src/` and the dashboard's TypeScript. Tests are left out, as repost-go leaves out `_test.go`, including a module's unit tests in `src/**/tests.rs`: they state expected screens literally. `dashboard/gauntlet/duplication.ts` passes jscpd its options and refuses a `.jscpd.json` or `package.json` key jscpd would merge. On offense: move the shared code into the module that owns it and call it from both places.

### Coverage floor

At least 90% of lines covered per crate (`cargo llvm-cov` with the browser tests, judged by `at-gauntlet coverage`) and in the dashboard (`vitest --coverage`). A branch nothing can reach is deleted, not excluded. A branch the operating system can reach stays and gets a test: an interrupted read, a full pipe, a pseudo-terminal that closes, a file that cannot be opened. Drive it through a fake at the library's wrapper (`native/pty.rs` hands out the reader and writer a fake can stand in for) rather than deleting it. On offense: test the uncovered behaviour through the crate's real surface.

### CRAP

CRAP(f) = complexity² × (1 − coverage)³ + complexity, at most 6 for every function: Rust through `at-gauntlet crap`, the dashboard through `gauntlet/crap.ts`. Complexity has one definition in both: 1 plus one per decision, where each `if`, loop, `&&`, `||` and `let … else` (Rust) or conditional, `??` and optional chain (TypeScript) is a decision, a `match` or `switch` counts once however many arms it has, and each match guard counts one more. For the dashboard that is ESLint's modified complexity, so a dispatch stays one readable `match` or `switch`. Coverage is the share of a function's code that ran: its llvm-cov code regions in Rust (`cargo llvm-cov report --json`), its statements in the dashboard, so an arm that never ran counts against its function even on a line that did. A function none of whose regions or statements ran fails whatever its complexity: one `match` scores exactly 6 with no test at all. A fully covered function passes up to complexity 6. On offense: cover every branch, or split the function into simpler ones that are each covered.

### Mutation

Every mutant on a line the change touched is caught. The area diffs `HEAD` against its merge-base with `main` (set `GAUNTLET_MUTATION_BASE` to the branch the change goes into), then runs cargo-mutants 27.1.0 with `--in-diff` in throwaway worktrees at `HEAD` for each crate the diff reaches, and Stryker 10.0.0 on the changed line ranges of the dashboard's source. The tests a mutant runs are the crate's whole suite, its unit tests first, so a mutant the unit tests catch costs seconds. `at-gauntlet mutation` fails a missed mutant, a baseline that did not pass, a report that tested fewer mutants than it listed, mutants that compiled with none caught, or timeouts over 5%. Stryker runs the dashboard's vitest suite with the TypeScript checker discarding mutants that do not compile, and `gauntlet/mutation.ts` fails a survivor, a mutant no test ran, an ignored mutant, mutants that compiled with none tested, timeouts over 5%, or a Stryker config file. A change that touches no line with a mutant passes. On offense: write a test whose expected value changes when the mutated code does, through the real surface.

### Acceptance

Black-box Gherkin features in `cli/acceptance/features` drive the built binary. `at-gauntlet gherkin` fails a feature without its current IR in `features/ir.json`, a `Then` step not bound through `then(…)` to a step that asserts, or a scenario marked `@wip`. The `acceptance-faults` area injects each named fault into the binary and fails unless the suite catches it. Each fault is a pattern for one cargo-mutants mutant, so the `rust` area lists the mutants with `cargo mutants --list` (under a second) and fails a fault that names none or more than one: a fault left pointing at code that moved fails at commit time, not hours later in `acceptance-faults`. On offense: bind the step to an assertion, regenerate the IR with `at-gauntlet gherkin-ir`, or add the scenario that would have caught the fault.

### Dashboard lint

`eslint --max-warnings 0` with the sealed config. In the dashboard's tests, `eslint-plugin-testing-library` holds `no-container`, `no-node-access` and `prefer-screen-queries` at error: a test finds what it checks the way a user does, by role, accessible name, label or text through `screen` or `within`, and asserts focus with `toHaveFocus()`. On offense: fix the code each finding names; an element with no role a user could find it by gets the role or name it lacks, and a purely decorative one a `data-testid`.

### Sealed ESLint config

`dashboard/eslint/sealed-config.test.ts`: every linted file gets every required rule and preset rule at error, every test file gets the three testing-library rules at error, inline config stays off, and both TypeScript projects keep every strict flag. On offense: restore the rule in `eslint.config.js` and fix the code it flags.

### Dashboard types

`tsc --noEmit` for the app, the ESLint tooling and the gauntlet tooling. On offense: fix the type errors.

### Gauntlet tools

`node --test` for the dashboard's ESLint rules and gauntlet scripts. On offense: fix the tool the failing test names.

### Dashboard tests

`vitest run --coverage`, failing under 90% of lines covered. On offense: fix the code the failing test names, or test the lines it reports uncovered.

### Dashboard build

`vite build` writes the app into `cli/assets/dashboard`, which the binary embeds and which must match what is committed. On offense: fix the build error, or commit the rebuilt bundle with its sources.

### Browser tests

`cargo test --features browser-tests --test cli browser::` drives the dashboard in Chrome through `agent-browser`. On offense: fix the dashboard behaviour the failing test names.

### npm tests

`node --test` over the npm launcher. On offense: fix the launcher code the failing test names.

### Catalogue

Every `GAUNTLET.md#<rule>` a check prints names a section of this file (`cli/tests/static_checks/catalogue.rs`). On offense: add the rule's section here, or point the check at the section that explains it.
