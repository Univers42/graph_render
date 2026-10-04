#!/usr/bin/env bash
# svc-digest-wasm.sh — the wasm32 arm of the digest manifest (`docs/contract/service-api.md`
# "Verdict", condition 7): every row of server/graph-server/tests/digest/manifest.json run
# through the real wasm artifact, compared with the digests the native arm committed.
#
#   scripts/orch/svc-digest-wasm.sh
#
# Two images, so two commands:
#
#   1. scripts/orch/gr  cargo build -p graph-wasm --release --target wasm32-unknown-unknown
#   2. scripts/orch/node-slim.sh node server/graph-server/tests/digest/wasm-arm.mjs <wasm> <manifest>
#
# `wasm-arm.mjs` prints one tab-separated line per row — fixture, source, layout, the POST id
# or `-`, then the SHA-256 of the framed snapshot bytes — and this script compares those lines
# with the manifest's own rows, sorted, so a difference is a diff and not a join.
#
# The build is the default artifact: no `threads`, no RUSTFLAGS. `scripts/orch/wasm-threads.sh`
# builds a different one, and comparing that here would be comparing two artifacts, not the
# wasm32 build against the native one.
#
# Exit: 0 every row agrees · 1 a row does not · 2 could not build or could not run
#
# Writes: nothing. `wasm-arm.mjs` reads the manifest and the two committed fixtures; this
#         script writes no file and needs no scratch copy.

set -uo pipefail
here=$(dirname "$(readlink -f "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root" || exit 2

# WHY the build's failure is 2 and not 1: a row that cannot run is not a row that disagrees.
# A `nonzero` gate row would be satisfied by a broken toolchain, so the two are kept apart.
wasm=target/wasm32-unknown-unknown/release/graph_wasm.wasm
manifest=server/graph-server/tests/digest/manifest.json

"$here/gr" cargo build -p graph-wasm --release --target wasm32-unknown-unknown || {
  echo "svc-digest-wasm: could not build $wasm" >&2
  exit 2
}
[[ -s $wasm ]] || {
  echo "svc-digest-wasm: no artifact at $wasm" >&2
  exit 2
}

got=$(mktemp) || exit 2
trap 'rm -f "$got"' EXIT
if ! "$here/node-slim.sh" node server/graph-server/tests/digest/wasm-arm.mjs "$wasm" "$manifest" >"$got"; then
  echo "svc-digest-wasm: could not run the wasm arm" >&2
  exit 2
fi

# The manifest's rows, spelled the way wasm-arm.mjs spells them. `python3` reads the committed
# file rather than a second copy of the row shape in this script: two spellings would drift,
# and a drift here would read as a motor difference.
want=$(mktemp) || exit 2
python3 - "$manifest" >"$want" <<'PY' || exit 2
import json, sys
for entry in json.load(open(sys.argv[1]))["entries"]:
    print("\t".join([entry["fixture"], entry["source"], entry["layout"],
                     entry["post"] or "-", entry["hash"]]))
PY

rows=$(wc -l <"$want")
if [[ $(wc -l <"$got") -ne $rows ]]; then
  echo "svc-digest-wasm: the wasm arm printed $(wc -l <"$got") rows, the manifest has $rows" >&2
  exit 1
fi
if ! diff -u <(sort "$want") <(sort "$got"); then
  echo "svc-digest-wasm: the wasm32 digests differ from the manifest above" >&2
  exit 1
fi
echo "svc-digest-wasm: $rows rows, wasm32 and native agree"