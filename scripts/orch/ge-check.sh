#!/usr/bin/env bash
# ge-check.sh — build and run the repo Dockerfile: the TypeScript oracle gate (npm run check).
set -euo pipefail
source "$(dirname "$(readlink -f "$0")")/docker-env.sh"
cd "$(git rev-parse --show-toplevel)"
docker build -q -t ge-check . >/dev/null
exec "$(dirname "$(readlink -f "$0")")/drun" --rm ge-check
