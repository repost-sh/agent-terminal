#!/bin/sh
set -eu

FLOOR=2.28

if [ $# -eq 0 ]; then
  echo "usage: glibc-floor.sh <gnu binary>...
  prints the newest GLIBC_ version each binary requires and fails when one is above $FLOOR" >&2
  exit 1
fi

newest() {
  sort -t . -k 1,1n -k 2,2n -k 3,3n | tail -n 1
}

status=0
for binary in "$@"; do
  required=$(objdump -p "$binary" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/^GLIBC_//' | newest)
  if [ -z "$required" ]; then
    echo "$binary: requires no GLIBC_ version, so it is not a gnu binary" >&2
    status=1
  elif [ "$(printf '%s\n%s\n' "$FLOOR" "$required" | newest)" = "$FLOOR" ]; then
    echo "$binary: newest required version GLIBC_$required, at or below the $FLOOR floor"
  else
    echo "$binary: newest required version GLIBC_$required, above the $FLOOR floor" >&2
    status=1
  fi
done

if [ "$status" -ne 0 ]; then
  echo "the gnu binaries must run on glibc $FLOOR: build them in the glibc $FLOOR images dist-workspace.toml names" >&2
fi
exit "$status"
