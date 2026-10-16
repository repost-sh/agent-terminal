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

run agent-terminal version
case $(cat "$out") in
  "agent-terminal $version ("*")") ;;
  *) fail "agent-terminal version printed '$(cat "$out")', expected 'agent-terminal $version (<commit>)'" ;;
esac

run agent-terminal --json version
case $(cat "$out") in
  "{\"build\":\"$version+"*"\",\"version\":\"$version\"}") ;;
  *) fail "agent-terminal --json version printed '$(cat "$out")', expected {\"build\":\"$version+<commit>\",\"version\":\"$version\"}" ;;
esac

account_home=$(getent passwd "$(id -u)" 2>/dev/null | cut -d: -f6)
[ -n "$account_home" ] || account_home=$(eval echo "~$(id -un)")
socket=$account_home/.agent-terminal/daemon.sock
[ ! -e "$socket" ] || fail "this check needs a machine with no agent-terminal daemon; $socket exists"

run agent-terminal session new install-check
run agent-terminal --session install-check open --name printer --size 40x10 --wait "line 3" "$fixture" --lines 3 --hold
run agent-terminal --session install-check snapshot
grep -q '^install-check printer · line-printer · 40x10 ·' "$out" || fail "the snapshot header does not name the fixture"
grep -q '^ 3│line 3$' "$out" || fail "the snapshot does not show the fixture's third line"
run agent-terminal close --all --session install-check
run agent-terminal daemon stop

echo "agent-terminal $version from $bin passed every check"
