#!/usr/bin/env bash
# studio-node.sh — run node in the node:22-slim image on the worktree, with the pinned
# references mounted read-only where the tests read them. TDD helper: one package, one glob.
#
#   scripts/studio-node.sh graph-render tests/camera.test.ts
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
pkg=${1:?the package: graph-render or graph-studio}
shift
exec docker run --rm -v "$root:/w" -w "/w/packages/$pkg" \
  -v "${REFS:-/goinfre/dlesieur/refs}:/refs:ro" \
  "${NODE_IMAGE:-node:22-slim}" node --test --test-reporter=tap "$@"
