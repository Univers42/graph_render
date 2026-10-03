#!/usr/bin/env bash
# studio-node.sh — run node in the node:22-slim image on the worktree, with the pinned
# references mounted read-only where the tests read them. TDD helper: one package, one glob.
#
#   scripts/studio-node.sh graph-render tests/camera.test.ts
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
pkg=${1:?the package: graph-render or graph-studio}
shift
source "$root/scripts/orch/image.sh"
exec "$root/scripts/orch/drun" --rm -v "$root:/w" -w "/w/packages/$pkg" \
  -v "${REFS:-$GM_SCRATCH/refs}:/refs:ro" \
  "$GM_NODE_IMAGE" node --test --test-reporter=tap "$@"
