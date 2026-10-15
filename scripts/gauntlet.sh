#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

fail() {
  printf '\ngauntlet: %s\n' "$1" >&2
  exit 1
}

step() {
  local fix="$1"
  shift
  printf '\n== %s\n' "$*"
  "$@" || fail "\`$*\` failed: $fix"
}

rust() {
  cd "$root/cli"
  step "run \`cargo fmt --all\` in cli/ → GAUNTLET.md#rust-format" cargo fmt --all --check
  step "fix the code each finding names; never allow a lint → GAUNTLET.md#rust-lint" cargo clippy --workspace --all-targets -- -D warnings
  step "fix the code each finding names; never allow a lint → GAUNTLET.md#rust-lint" cargo clippy --all-targets --features browser-tests -- -D warnings
  step "replace or update the crate cargo-deny names → GAUNTLET.md#dependency-policy" cargo deny check
  step "remove the dependency cargo-machete names from its Cargo.toml → GAUNTLET.md#unused-dependencies" cargo machete . gauntlet perf tests/fixtures
  step "fix the code the failing test or static check names → GAUNTLET.md#rust-tests" cargo test --workspace
  step "state an invariant of the crate's behaviour as a proptest! → GAUNTLET.md#property-tests" cargo run --quiet -p at-gauntlet -- props
}

coverage() {
  command -v cargo-llvm-cov >/dev/null || fail "cargo-llvm-cov is not on PATH: run \`cargo install cargo-llvm-cov --locked\` → GAUNTLET.md#coverage-floor"
  command -v agent-browser >/dev/null || fail "agent-browser is not on PATH: coverage runs the browser tests too; install it → GAUNTLET.md#coverage-floor"
  cd "$root/cli"
  step "fix the error cargo-llvm-cov printed → GAUNTLET.md#coverage-floor" cargo llvm-cov clean --workspace
  step "fix the code the failing test names → GAUNTLET.md#rust-tests" cargo llvm-cov --workspace --all-targets --features browser-tests --no-report
  step "fix the error cargo-llvm-cov printed → GAUNTLET.md#coverage-floor" cargo llvm-cov report --lcov --no-default-ignore-filename-regex --output-path target/lcov.info
  step "test the uncovered behaviour through the crate's real surface → GAUNTLET.md#coverage-floor" cargo run --quiet -p at-gauntlet -- coverage target/lcov.info
  step "cover every branch of each function it names, or split the function → GAUNTLET.md#crap" cargo run --quiet -p at-gauntlet -- crap target/lcov.info
}

npm_tests() {
  cd "$root"
  step "fix the launcher code the failing test names → GAUNTLET.md#npm-tests" node --test 'npm/test/*.test.js'
}

dashboard() {
  cd "$root/dashboard"
  [ -x node_modules/.bin/eslint ] || fail "dashboard/node_modules is missing: run \`npm ci\` in dashboard/"
  step "fix the code each finding names; never disable a rule → GAUNTLET.md#dashboard-lint" npm run --silent lint
  step "fix the type errors → GAUNTLET.md#dashboard-types" npm run --silent check
  step "fix the type errors in the lint tooling → GAUNTLET.md#dashboard-types" node_modules/.bin/tsc --noEmit -p eslint
  step "fix the type errors in the CRAP tool → GAUNTLET.md#dashboard-types" node_modules/.bin/tsc --noEmit -p gauntlet
  step "fix the rule or the sealed config the failing test names → GAUNTLET.md#sealed-eslint-config" node --test 'eslint/*.test.ts'
  step "fix the CRAP tool the failing test names → GAUNTLET.md#crap" node --test 'gauntlet/*.test.ts'
  step "fix the code the failing test names, or test the lines it reports uncovered → GAUNTLET.md#dashboard-tests" npm run --silent test
  step "cover every branch of each function it names, or split the function → GAUNTLET.md#crap" node gauntlet/crap.ts
  step "state an invariant of the dashboard's behaviour as fc.assert(fc.property(…)) → GAUNTLET.md#property-tests" node gauntlet/props.ts
  step "fix the build error → GAUNTLET.md#dashboard-build" npm run --silent build
  cd "$root"
  git diff --quiet -- cli/assets/dashboard || fail "the built dashboard differs from cli/assets/dashboard: commit the rebuilt cli/assets/dashboard with its sources → GAUNTLET.md#dashboard-build"
  [ -z "$(git ls-files --others --exclude-standard -- cli/assets/dashboard)" ] || fail "cli/assets/dashboard has untracked files: commit them with the sources → GAUNTLET.md#dashboard-build"
}

browser() {
  command -v agent-browser >/dev/null || fail "agent-browser is not on PATH: install it to run the browser tests → GAUNTLET.md#browser-tests"
  cd "$root/cli"
  step "fix the dashboard behaviour the failing browser test names → GAUNTLET.md#browser-tests" cargo test --features browser-tests --test cli browser::
}

areas=("$@")
if [ "${#areas[@]}" -eq 0 ]; then
  areas=(rust dashboard npm)
fi

for area in "${areas[@]}"; do
  case "$area" in
    rust) rust ;;
    dashboard) dashboard ;;
    npm) npm_tests ;;
    browser) browser ;;
    coverage) coverage ;;
    *) fail "unknown area \`$area\`: use rust, dashboard, npm, browser or coverage" ;;
  esac
done

printf '\nGAUNTLET GREEN\n'
