#!/bin/sh
# Supply chain (threat T7): every action a workflow uses must be pinned to a
# full commit hash. A tag such as @v4 can later point to other code.
set -e
bad=$(grep -nE '^[[:space:]]*(-[[:space:]]+)?uses:' .github/workflows/*.yml | grep -vE 'uses:[[:space:]]+[^@[:space:]]+@[0-9a-f]{40}([[:space:]]|$)' || true)
if [ -n "$bad" ]; then
  echo "$bad"
  echo "FAIL: the actions above are not pinned to a commit hash." >&2
  exit 1
fi
echo "OK: every action is pinned to a commit hash."
