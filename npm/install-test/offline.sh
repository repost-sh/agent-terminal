#!/bin/sh
set -eu

PROBE=https://registry.npmjs.org/

if [ $# -eq 0 ]; then
  echo "usage: offline.sh <command> [args...]: run the command with networking cut" >&2
  exit 1
fi

if [ "$1" = --cut ]; then
  shift
  if node -e "fetch('$PROBE').then(() => process.exit(0), () => process.exit(1))"; then
    echo "networking is not cut: $PROBE answered" >&2
    exit 1
  fi
  echo "networking is cut: $PROBE is unreachable"
  exec "$@"
fi

case $(uname -s) in
  Darwin)
    exec sandbox-exec -p '(version 1)(allow default)(deny network-outbound (remote ip))' sh "$0" --cut "$@"
    ;;
  Linux)
    if [ "$(id -u)" -eq 0 ]; then
      exec unshare --net -- sh "$0" --cut "$@"
    fi
    exec sudo --preserve-env env "PATH=$PATH" unshare --net -- \
      setpriv --reuid="$(id -u)" --regid="$(id -g)" --init-groups -- sh "$0" --cut "$@"
    ;;
  *)
    echo "offline.sh cannot cut networking on $(uname -s)" >&2
    exit 1
    ;;
esac
