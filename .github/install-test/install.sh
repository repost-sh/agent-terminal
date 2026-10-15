#!/bin/sh
set -eu

PNPM_VERSION=12.11.2
BUN_VERSION=1.4.3

fail() {
  printf '%s\n' "$*" >&2
  exit 1
}

if [ $# -lt 3 ] || [ $# -gt 4 ]; then
  fail "usage: install.sh <npm|pnpm|bun|archive> <artifacts> <state> [expected npm platform package]
  <artifacts> holds npm/ (the npm tarballs) and archive/ (one release archive and its .sha256);
  install.sh records the bin folder in <state>/bin-dir"
fi
channel=$1
artifacts=$(cd "$2" && pwd)
mkdir -p "$3"
state=$(cd "$3" && pwd)
expected=${4:-}
here=$(cd "$(dirname "$0")" && pwd)

from_local_registry() {
  node "$here/registry.js" "$artifacts/npm" "$@"
}

tool() {
  npm install --global --prefix "$state/tools" --no-audit --no-fund "$1"
}

case $channel in
  npm)
    from_local_registry npm install --global --prefix "$state/npm" --no-audit --no-fund @repost/agent-terminal
    installed=$(cd "$state/npm/lib/node_modules/@repost/agent-terminal/node_modules" && ls -d @*/*)
    if [ -n "$expected" ] && [ "$installed" != "$expected" ]; then
      fail "npm installed the binary packages: $installed (expected $expected only)"
    fi
    bin=$state/npm/bin
    ;;
  pnpm)
    tool "pnpm@$PNPM_VERSION"
    PNPM_HOME=$state/pnpm
    PATH=$PNPM_HOME/bin:$PATH
    export PNPM_HOME PATH
    from_local_registry "$state/tools/bin/pnpm" add --global @repost/agent-terminal
    bin=$PNPM_HOME/bin
    ;;
  bun)
    tool "bun@$BUN_VERSION"
    BUN_INSTALL=$state/bun
    export BUN_INSTALL
    from_local_registry "$state/tools/bin/bun" add --global @repost/agent-terminal
    bin=$BUN_INSTALL/bin
    ;;
  archive)
    cd "$artifacts/archive"
    set -- agent-terminal-*.tar.xz
    [ $# -eq 1 ] && [ -f "$1" ] || fail "expected one release archive in $artifacts/archive, found: $*"
    archive=$1
    if command -v sha256sum > /dev/null; then
      actual=$(sha256sum "$archive" | cut -d' ' -f1)
    else
      actual=$(shasum -a 256 "$archive" | cut -d' ' -f1)
    fi
    expected_sum=$(cut -d' ' -f1 "$archive.sha256")
    [ "$actual" = "$expected_sum" ] || fail "$archive has sha256 $actual, but $archive.sha256 says $expected_sum"
    mkdir -p "$state/archive"
    tar -xJf "$archive" -C "$state/archive"
    bin=$state/archive/${archive%.tar.xz}
    ;;
  *)
    fail "unknown channel $channel: expected npm, pnpm, bun or archive"
    ;;
esac

[ -x "$bin/agent-terminal" ] || fail "the $channel install left no agent-terminal in $bin"
printf '%s\n' "$bin" > "$state/bin-dir"
echo "installed agent-terminal with $channel into $bin"
