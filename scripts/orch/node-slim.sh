#!/usr/bin/env bash
# node-slim.sh <cmd...> — run <cmd> in node:22-slim (pinned: GM_NODE_IMAGE, image.sh) with the git
# top-level mounted at /w.
set -euo pipefail
source "$(dirname "$(readlink -f "$0")")/image.sh"
exec docker run --rm -e NPM_CONFIG_UPDATE_NOTIFIER=false -v "$(git rev-parse --show-toplevel):/w" -w /w \
  "$GM_NODE_IMAGE" "$@"
