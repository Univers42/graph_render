#!/usr/bin/env bash
# run.sh <repo> [<name>]: a repository to a studio document, printed as its path. <name> is the
# ingest source and defaults to the repository directory's name without `.git`. Work files go
# under target/git-plugin/<name>/ because node-slim and gr see only the worktree (/w).
set -euo pipefail
repo=$(cd "$1" && pwd)
name=${2:-$(basename "$repo" .git)}
root=$(git rev-parse --show-toplevel)
here=examples/plugins/git
work=target/git-plugin/$name
mkdir -p "$root/$work"
cd "$root"
"$here/git-log.sh" "$repo" > "$work/log"
scripts/orch/node-slim.sh node --experimental-strip-types "$here/export.mjs" "$name" "$work/log" "$work/ingest.json"
CARGO_BUILD_JOBS=3 scripts/orch/gr cargo run -q --release -p graph-cli -- ingest \
  --from "$work/ingest.json" --member ingest --out "$work/graph.json"
scripts/orch/node-slim.sh node "$here/decorate.mjs" "$work/ingest.json" "$work/graph.json" refs "$work/$name.studio.json"
echo "$work/$name.studio.json"