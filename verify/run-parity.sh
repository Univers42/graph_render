#!/usr/bin/env bash
#
# Visual-parity verification for the extracted graph-engine.
#
# Answers one question with evidence rather than assertion: does the standalone
# package render the same as the engine still sitting in osionos?
#
# WHAT IT DOES
#   1. Extracts the host's real `--osio-*` token values from osionos'
#      src/app/styles/global.css into a dependency-free stylesheet. The canvas reads
#      its colours with getComputedStyle, so this reproduces exactly the surface the
#      engine sees — no Tailwind build required.
#   2. Serves verify/parity, which mounts THREE engines in one document: the host's
#      in-tree copy twice and the standalone once, all fed the identical model.
#   3. Compares the two graph canvases and the two background canvases pixel by
#      pixel, for every one of the host's 7 palettes x light/dark.
#   4. Reports a verdict with an explicit exit code.
#
# WHY IT NEEDS ITS OWN CONTAINER
#   The VM has no browser and cannot install one (no writable /opt, and sudo needs a
#   tty), and osionos' browser-tests image carries a full pnpm install. This uses the
#   official Playwright image plus two dev dependencies. vite is installed HERE and
#   never added to package.json: the package deliberately ships no bundler, because a
#   bundler is the consumer's choice.
#
# EXIT CODES
#   0  PIXEL-IDENTICAL   host and standalone agree, in every palette, with the graph
#                        demonstrably drawn and the tokens demonstrably resolved
#   2  DIVERGENT         a real difference was measured
#   1  INCONCLUSIVE      the rig could not run cleanly (or drew nothing, or the
#                        tokens did not resolve, or the sweep did not discriminate)
#
# The 1-vs-2 split matters: "I could not tell" is not "they match", and an earlier
# revision of this rig reported success while comparing an empty canvas.

set -euo pipefail

SELF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOST_APP="${HOST_APP:-/home/dlesieur/Documents/osionos}"
IMAGE="${PARITY_IMAGE:-ge-parity-rig}"
CONTAINER="${PARITY_CONTAINER:-ge-parity-rig}"
PORT="${PARITY_PORT:-5555}"

GLOBAL_CSS="$HOST_APP/src/app/styles/global.css"
IN_TREE="$HOST_APP/packages/graph-engine/src"

die() { printf '\n[parity] ERROR: %s\n' "$1" >&2; exit 1; }

[ -f "$GLOBAL_CSS" ] || die "host stylesheet not found: $GLOBAL_CSS"
[ -d "$IN_TREE" ]    || die "host in-tree engine not found: $IN_TREE"

printf '[parity] extracting host token values from %s\n' "$GLOBAL_CSS"
node "$SELF/verify/extract-tokens.mjs" "$GLOBAL_CSS" "$SELF/verify/parity/tokens.host.css"

printf '[parity] building rig image (playwright + vite, no host npm)\n'
docker build -q -f "$SELF/verify/Dockerfile.rig" -t "$IMAGE" "$SELF" >/dev/null

docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
printf '[parity] starting rig on 127.0.0.1:%s\n' "$PORT"
docker run -d --name "$CONTAINER" \
  -p "127.0.0.1:${PORT}:5555" \
  -v "$SELF:/work" \
  -v "$HOST_APP:/osionos:ro" \
  -e SELF=/work -e HOST_APP=/osionos \
  "$IMAGE" \
  sh -c 'npx vite --config /work/verify/vite.verify.config.ts --host 0.0.0.0 --port 5555' \
  >/dev/null

cleanup() {
  printf '\n[parity] artifacts:\n'
  ls -1 "$SELF"/verify/*.png 2>/dev/null | sed 's/^/  /' || true
  printf '[parity] stopping rig\n'
  docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Wait for the dev server rather than sleeping a fixed amount.
for _ in $(seq 1 60); do
  if curl -fsS -o /dev/null "http://127.0.0.1:${PORT}/"; then break; fi
  sleep 1
done
curl -fsS -o /dev/null "http://127.0.0.1:${PORT}/" || die "rig did not come up (docker logs $CONTAINER)"

printf '[parity] running check across 7 palettes x light/dark\n'
# The script is copied into the rig so its imports resolve against the rig's own
# node_modules rather than the package's (which has no bundler, by design).
set +e
docker exec "$CONTAINER" sh -c \
  'cp /work/verify/parity-check.mjs /rig/ && node /rig/parity-check.mjs'
STATUS=$?
set -e

case "$STATUS" in
  0) printf '\n[parity] PIXEL-IDENTICAL — extraction verified in every palette\n' ;;
  2) printf '\n[parity] DIVERGENT — the standalone renders differently; see JSON above\n' ;;
  *) printf '\n[parity] INCONCLUSIVE — the rig could not produce a trustworthy result\n' ;;
esac
exit "$STATUS"
