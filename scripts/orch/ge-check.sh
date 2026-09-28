#!/usr/bin/env bash
# ge-check.sh — build and run the repo Dockerfile: the TypeScript oracle gate (npm run check).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
docker build -q -t ge-check . >/dev/null
exec docker run --rm ge-check
