#!/usr/bin/env bash
# node-slim.sh <cmd...> — run <cmd> in node:22-slim with the git top-level mounted at /w.
set -euo pipefail
source "$(dirname "$(readlink -f "$0")")/docker-env.sh"
exec docker run --rm -v "$(git rev-parse --show-toplevel):/w" -w /w node:22-slim "$@"
