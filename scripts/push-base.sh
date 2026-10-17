#!/usr/bin/env bash
set -euo pipefail

before="${1:-}"

if [[ "$before" =~ ^0*$ ]]; then
  echo "push-base: no commit came before this push: it created the branch, or none was given" >&2
  exit 1
fi

if ! git cat-file -e "$before^{commit}" 2>/dev/null; then
  echo "push-base: '$before' is not a commit in this clone, as after a force-push" >&2
  exit 1
fi

printf '%s\n' "$before"
