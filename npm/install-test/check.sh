#!/bin/sh
set -eu

fail() {
  printf '%s\n' "$*" >&2
  exit 1
}

if [ $# -ne 3 ]; then
  fail "usage: check.sh <state> <expected version> <line-printer fixture>
  <state> is the folder install.sh recorded the bin folder in"
fi
state=$1
version=$2
fixture=$3
bin=$(cat "$state/bin-dir")
PATH=$bin:$PATH
export PATH
work=$(mktemp -d "$state/check.XXXXXX")
out=$work/out

run() {
  if "$@" < /dev/null > "$out" 2>&1; then
    cat "$out"
  else
    cat "$out" >&2
    fail "failed: $*"
  fi
}

found=$(command -v agent-terminal)
[ "$found" = "$bin/agent-terminal" ] || fail "agent-terminal resolves to $found, not to the one installed in $bin"

run agent-terminal --version
[ "$(cat "$out")" = "agent-terminal $version" ] || fail "agent-terminal --version printed '$(cat "$out")', expected 'agent-terminal $version'"

run agent-terminal --json version
[ "$(cat "$out")" = "{\"version\":\"$version\"}" ] || fail "agent-terminal --json version printed '$(cat "$out")', expected {\"version\":\"$version\"}"

run env HOME="$work/home" agent-terminal install --skills
stub=$work/home/.claude/skills/agent-terminal/SKILL.md
grep -qx "  version: \"$version\"" "$stub" || fail "the skill stub install --skills wrote does not carry version $version: $(grep -n 'version:' "$stub" || true)"

AGENT_TERMINAL_HOME=$work/at
export AGENT_TERMINAL_HOME
run agent-terminal open --size 40x10 --wait "line 3" "$fixture" --lines 3 --hold
run agent-terminal snapshot
grep -q '^default t1 · line-printer · 40x10 ·' "$out" || fail "the snapshot header does not name the fixture"
grep -q '^ 3│line 3$' "$out" || fail "the snapshot does not show the fixture's third line"
run agent-terminal close
run agent-terminal daemon stop
agent-terminal doctor < /dev/null > "$out" 2>&1 || true
cat "$out"
grep -q '^ok  *daemon  *not running' "$out" || fail "the daemon is still running after daemon stop"

echo "agent-terminal $version from $bin passed every check"
