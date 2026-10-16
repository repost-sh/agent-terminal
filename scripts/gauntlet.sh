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
  step "remove the dependency cargo-machete names from its Cargo.toml → GAUNTLET.md#unused-dependencies" cargo machete . acceptance gauntlet perf tests/fixtures
  step "fix the code the failing test or static check names → GAUNTLET.md#rust-tests" cargo test --workspace
  step "state an invariant of the crate's behaviour as a proptest! → GAUNTLET.md#property-tests" cargo run --quiet -p at-gauntlet -- props
  step "move what both layers need into a contract module, and give every module and crate its place in the policy → GAUNTLET.md#architecture" cargo run --quiet -p at-gauntlet -- architecture
  step "group the parameters into one struct with named fields, or keep the function inside its module → GAUNTLET.md#parameter-ceiling" cargo run --quiet -p at-gauntlet -- params
  step "bind each Then step to a step that asserts and regenerate the IR with \`at-gauntlet gherkin-ir\` → GAUNTLET.md#acceptance" cargo run --quiet -p at-gauntlet -- gherkin
  mutants_installed
  printf '\n== the acceptance faults each name one mutant of cargo mutants --list\n'
  faults_named
}

coverage() {
  command -v cargo-llvm-cov >/dev/null || fail "cargo-llvm-cov is not on PATH: run \`cargo install cargo-llvm-cov --locked\` → GAUNTLET.md#coverage-floor"
  command -v agent-browser >/dev/null || fail "agent-browser is not on PATH: coverage runs the browser tests too; install it → GAUNTLET.md#coverage-floor"
  cd "$root/cli"
  step "fix the error cargo-llvm-cov printed → GAUNTLET.md#coverage-floor" cargo llvm-cov clean --workspace
  step "fix the code the failing test names → GAUNTLET.md#rust-tests" cargo llvm-cov --workspace --all-targets --features browser-tests --no-report
  step "fix the error cargo-llvm-cov printed → GAUNTLET.md#coverage-floor" cargo llvm-cov report --lcov --no-default-ignore-filename-regex --output-path target/lcov.info
  step "fix the error cargo-llvm-cov printed → GAUNTLET.md#crap" cargo llvm-cov report --json --no-default-ignore-filename-regex --output-path target/coverage.json
  step "test the uncovered behaviour through the crate's real surface → GAUNTLET.md#coverage-floor" cargo run --quiet -p at-gauntlet -- coverage target/lcov.info
  step "cover every branch of each function it names, or split the function → GAUNTLET.md#crap" cargo run --quiet -p at-gauntlet -- crap target/coverage.json
}

MUTANTS_VERSION=27.1.0

FAULTS=(
  'src/commands/values/mod\.rs:.*: replace escaped -> Result<u8, String> with Ok\(0\)$'
  'src/native/daemon/mod\.rs:.*: replace Daemon::list -> Sessions with Default::default\(\)$'
  'src/native/keyboard/mod\.rs:.*: delete match arm \(1, false\) in letter$'
  'src/native/terminal/typing\.rs:.*: replace Terminal::press -> Result<usize> with Ok\(0\)$'
  'src/native/daemon/waiting/mod\.rs:.*: replace Until<[^>]*>::wait_on -> Result<u64, ErrorBody> with Ok\(0\)$'
  'src/native/screen/rows\.rs:.*: replace plain_row -> String with "xyzzy"\.into\(\)$'
)

mutants_installed() {
  command -v cargo-mutants >/dev/null && [ "$(cargo mutants --version)" = "cargo-mutants $MUTANTS_VERSION" ] ||
    fail "cargo-mutants $MUTANTS_VERSION is not on PATH: run \`cargo install cargo-mutants --version $MUTANTS_VERSION --locked\` → GAUNTLET.md#mutation"
}

faults_named() {
  local listed fault
  listed="$(cargo mutants --list -p agent-terminal)" || fail "\`cargo mutants --list\` failed: fix the error it printed → GAUNTLET.md#acceptance"
  for fault in "${FAULTS[@]}"; do
    [ "$(grep -c -E -- "$fault" <<<"$listed")" = 1 ] ||
      fail "the fault \`$fault\` names no single mutant: point it at the code that moved → GAUNTLET.md#acceptance"
  done
  printf 'each of the %s acceptance faults names one mutant\n' "${#FAULTS[@]}"
}

fresh_tree() {
  git -C "$root" worktree prune
  if [ -e "$1" ]; then
    git -C "$root" worktree remove --force "$1"
  fi
  git -C "$root" worktree add --quiet --detach "$1" HEAD
}

mutate_shard() {
  local crate="$1" shard="$2" tests="$1"
  local k="${shard%%/*}"
  case "$crate" in
    at-acceptance | at-fixtures) tests=agent-terminal ;;
  esac
  cd "$work/w$k/cli"
  CARGO_BUILD_BUILD_DIR="$root/cli/target/mb$k" cargo mutants --in-place -p "$crate" --test-package "$tests" \
    --in-diff "$work/rust.diff" --shard "$shard" --sharding round-robin -o "$work/out/$crate-$k" >"$work/out/$crate-$k.log" 2>&1
}

mutation() {
  mutants_installed
  local base="${GAUNTLET_MUTATION_BASE:-main}" most="${GAUNTLET_MUTATION_SHARDS:-4}" since listed shards reports=()
  since="$(git -C "$root" merge-base HEAD "$base")" ||
    fail "HEAD shares no commit with $base: set GAUNTLET_MUTATION_BASE to the branch the change goes into → GAUNTLET.md#mutation"
  work="$root/cli/target/mutation"
  mkdir -p "$work/out"
  git -C "$root" diff "$since" HEAD --relative=cli -- cli >"$work/rust.diff"
  git -C "$root" diff "$since" HEAD --relative=dashboard -- dashboard >"$work/dashboard.diff"
  for ((k = 0; k < most; k++)); do
    fresh_tree "$work/w$k"
  done
  for crate in agent-terminal at-acceptance at-fixtures at-gauntlet at-perf; do
    listed="$(cd "$work/w0/cli" && cargo mutants --list -p "$crate" --in-diff "$work/rust.diff" | wc -l)"
    shards=$((listed < most ? listed : most))
    printf '%s: %d mutants on the lines changed since %s\n' "$crate" "$listed" "${since:0:12}"
    for ((k = 0; k < shards; k++)); do
      rm -rf "$work/out/$crate-$k"
      mkdir -p "$work/out/$crate-$k"
      reports+=("$work/out/$crate-$k/mutants.out/outcomes.json")
      (mutate_shard "$crate" "$k/$shards") &
    done
    wait
  done
  cd "$root/cli"
  if [ "${#reports[@]}" -gt 0 ]; then
    step "write a test whose expected value changes when the mutated line does → GAUNTLET.md#mutation" cargo run --quiet -p at-gauntlet -- mutation "${reports[@]}"
  else
    printf 'no Rust line with a mutant changed since %s\n' "${since:0:12}"
  fi
  cd "$root/dashboard"
  [ -x node_modules/.bin/stryker ] || fail "dashboard/node_modules is missing: run \`npm ci\` in dashboard/ → GAUNTLET.md#mutation"
  step "write a test whose expected value changes when the mutated code does → GAUNTLET.md#mutation" node gauntlet/mutation.ts "$work/dashboard.diff"
}

acceptance_faults() {
  mutants_installed
  local picked=()
  work="$root/cli/target/acceptance-faults"
  fresh_tree "$work/tree"
  cd "$work/tree/cli"
  mkdir -p "$work/out"
  faults_named
  for fault in "${FAULTS[@]}"; do
    picked+=(--re "$fault")
  done
  if ! CARGO_BUILD_BUILD_DIR="$root/cli/target/mbf" cargo mutants --in-place -p agent-terminal "${picked[@]}" --minimum-test-timeout 120 -o "$work/out" -- --test acceptance >"$work/out.log" 2>&1; then
    printf 'cargo-mutants reported a missed mutant; it also runs struct-field mutants that --re cannot leave out, so the next step judges only the baseline and the named faults\n'
  fi
  cd "$root/cli"
  step "add the scenario that would have caught the fault it names → GAUNTLET.md#acceptance" cargo run --quiet -p at-gauntlet -- faults "$work/out/mutants.out/outcomes.json" "${FAULTS[@]}"
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
  step "fix the gauntlet tool the failing test names → GAUNTLET.md#gauntlet-tools" node --test 'gauntlet/*.test.ts'
  step "fix the code the failing test names, or test the lines it reports uncovered → GAUNTLET.md#dashboard-tests" npm run --silent test
  step "cover every branch of each function it names, or split the function → GAUNTLET.md#crap" node gauntlet/crap.ts
  step "state an invariant of the dashboard's behaviour as fc.assert(fc.property(…)) → GAUNTLET.md#property-tests" node gauntlet/props.ts
  step "fix the build error → GAUNTLET.md#dashboard-build" npm run --silent build
  cd "$root"
  git diff --quiet -- cli/assets/dashboard || fail "the built dashboard differs from cli/assets/dashboard: commit the rebuilt cli/assets/dashboard with its sources → GAUNTLET.md#dashboard-build"
  [ -z "$(git ls-files --others --exclude-standard -- cli/assets/dashboard)" ] || fail "cli/assets/dashboard has untracked files: commit them with the sources → GAUNTLET.md#dashboard-build"
}

duplication() {
  cd "$root/dashboard"
  [ -x node_modules/.bin/jscpd ] || fail "dashboard/node_modules is missing: run \`npm ci\` in dashboard/ → GAUNTLET.md#duplication"
  step "move the shared code into the module that owns it and call it from both places → GAUNTLET.md#duplication" node gauntlet/duplication.ts
}

browser() {
  command -v agent-browser >/dev/null || fail "agent-browser is not on PATH: install it to run the browser tests → GAUNTLET.md#browser-tests"
  cd "$root/cli"
  step "fix the dashboard behaviour the failing browser test names → GAUNTLET.md#browser-tests" cargo test --features browser-tests --test cli browser::
}

areas=("$@")
if [ "${#areas[@]}" -eq 0 ]; then
  areas=(rust dashboard npm duplication)
fi

for area in "${areas[@]}"; do
  case "$area" in
    rust) rust ;;
    dashboard) dashboard ;;
    npm) npm_tests ;;
    duplication) duplication ;;
    browser) browser ;;
    coverage) coverage ;;
    mutation) mutation ;;
    acceptance-faults) acceptance_faults ;;
    *) fail "unknown area \`$area\`: use rust, dashboard, npm, duplication, browser, coverage, mutation or acceptance-faults" ;;
  esac
done

printf '\nGAUNTLET GREEN\n'
