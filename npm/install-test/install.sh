#!/bin/sh
set -eu

PNPM_VERSION=12.11.2
BUN_VERSION=1.4.3

fail() {
  printf '%s\n' "$*" >&2
  exit 1
}

if [ $# -lt 3 ] || [ $# -gt 4 ]; then
  fail "usage: install.sh <npm|pnpm|bun|archive|homebrew> <artifacts> <state> [expected npm platform package]
  <artifacts> holds npm/ (the npm tarballs), archive/ (one release archive and its .sha256)
  and formula/ (the Homebrew formula dist generated); install.sh records the bin folder in <state>/bin-dir"
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

brew_command() {
  for candidate in "$(command -v brew || true)" /opt/homebrew/bin/brew /usr/local/bin/brew /home/linuxbrew/.linuxbrew/bin/brew; do
    if [ -n "$candidate" ] && [ -x "$candidate" ]; then
      printf '%s\n' "$candidate"
      return
    fi
  done
  fail "Homebrew is not installed"
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
      checksum=sha256sum
    else
      checksum="shasum -a 256"
    fi
    grep -v '^$' "$archive.sha256" | $checksum -c -
    mkdir -p "$state/archive"
    tar -xJf "$archive" -C "$state/archive"
    bin=$state/archive/${archive%.tar.xz}
    ;;
  homebrew)
    brew=$(brew_command)
    formula=$artifacts/formula/agent-terminal.rb
    sed "s|\"https://[^\"]*/\(agent-terminal-[^\"/]*\.tar\.xz\)\"|\"file://$artifacts/archive/\1\"|" "$formula" > "$state/agent-terminal.rb"
    grep -q "file://$artifacts/archive/" "$state/agent-terminal.rb" || fail "$formula has no release archive URL to point at $artifacts/archive"
    export HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_INSTALL_CLEANUP=1
    "$brew" tap-new --no-git clean-install/local
    cp "$state/agent-terminal.rb" "$("$brew" --repository)/Library/Taps/clean-install/homebrew-local/Formula/agent-terminal.rb"
    "$brew" install clean-install/local/agent-terminal
    bin=$("$brew" --prefix)/bin
    ;;
  *)
    fail "unknown channel $channel: expected npm, pnpm, bun, archive or homebrew"
    ;;
esac

[ -x "$bin/agent-terminal" ] || fail "the $channel install left no agent-terminal in $bin"
printf '%s\n' "$bin" > "$state/bin-dir"
echo "installed agent-terminal with $channel into $bin"
