#!/bin/sh
set -eu

if [ $# -ne 3 ]; then
  echo "usage: run.sh <npm-tarball-dir> <line-printer-fixture> <expected-platform-package>" >&2
  exit 1
fi
tarballs=$1
fixture=$2
expected=$3
here=$(cd "$(dirname "$0")" && pwd)

node "$here/registry.js" "$tarballs" npm install --global --no-audit --no-fund @repost/agent-terminal

installed=$(cd "$(npm root --global)/@repost/agent-terminal/node_modules" && ls -d @*/*)
if [ "$installed" != "$expected" ]; then
  echo "npm installed the binary packages: $installed (expected $expected only)" >&2
  exit 1
fi

agent-terminal open --size 40x10 --wait "line 3" "$fixture" --lines 3 --hold
screen=$(agent-terminal snapshot)
printf '%s\n' "$screen"
printf '%s\n' "$screen" | grep -q '^default t1 · line-printer · 40x10 ·' || {
  echo "the snapshot header does not name the fixture" >&2
  exit 1
}
printf '%s\n' "$screen" | grep -q '^ 3│line 3$' || {
  echo "the snapshot does not show the fixture's third line" >&2
  exit 1
}
agent-terminal close
