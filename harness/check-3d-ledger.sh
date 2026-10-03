#!/usr/bin/env bash
# The five 3D ids, read back through the registry the way the studio picker reads it.
#
#   scripts/orch/gr bash /w/harness/check-3d-ledger.sh
#
# Three things are checked per id, and each is a different claim:
#
# 1. the id is a **layout** row at all — a typo in the registry would otherwise leave this
#    script passing over a set that no longer contains it;
# 2. its status is `implemented`, never `gated`: a force row may only claim `gated` behind
#    a recorded oracle run, and no 3D arm has one on this tree (`unproven.rs` gives the
#    argument, including why `verdict::Evidence::oracle_record` has no arm for
#    `oracle-igraph3d`);
# 3. its `oracle` text names the oracle its own differential uses, so the row cannot drift
#    onto another arm's oracle by accident.
#
# Pure grep, not a JSON parser: the ge-rust image has no `python3` (`p12-t2.rows` records the
# same), and `capabilities --json` is pretty-printed one field per line, so an id and its
# status are a few lines apart and a record is a short window.
#
# The `oracle_record` field itself is not in the JSON; that is asserted by
# `crates/graph-cli/src/capabilities/tests/registry.rs`, in the workspace suite.
set -euo pipefail

CARGO=${GM_CARGO:-cargo}
OUT=${GM_CAPS_OUT:-/w/target/capabilities.json}
mkdir -p "$(dirname "$OUT")"

"$CARGO" run -q -p graph-cli -- capabilities --json > "$OUT"

IDS="layout.forceatlas2.3d layout.force.fruchterman_reingold.3d
layout.force.kamada_kawai.3d layout.force.drl.3d layout.force.yifan_hu.2z"

for id in $IDS; do
  if ! grep -q "\"id\": \"$id\"" "$OUT"; then
    echo "check-3d-ledger: $id is not a registered row" >&2
    exit 1
  fi
  if ! grep -A8 "\"id\": \"$id\"" "$OUT" | grep -q '"stage": "layout"'; then
    echo "check-3d-ledger: $id is not a layout row" >&2
    exit 1
  fi
  if ! grep -A8 "\"id\": \"$id\"" "$OUT" | grep -q '"status": "implemented"'; then
    echo "check-3d-ledger: $id is not 'implemented' (a force row may not claim gated)" >&2
    exit 1
  fi
done

# The oracle each arm's own differential uses, as it must appear in the row's own text.
check_oracle() {
  id=$1
  needle=$2
  if ! grep -A12 "\"id\": \"$id\"" "$OUT" | grep -q "$needle"; then
    echo "check-3d-ledger: $id does not name $needle in its oracle" >&2
    exit 1
  fi
}

check_oracle layout.forceatlas2.3d 'oracle-fa2'
check_oracle layout.force.fruchterman_reingold.3d 'oracle-igraph3d'
check_oracle layout.force.kamada_kawai.3d 'oracle-igraph3d'
check_oracle layout.force.drl.3d 'oracle-igraph3d'
# `2Z` has no coordinate oracle and must say so rather than borrow one.
check_oracle layout.force.yifan_hu.2z 'none is claimed'

echo "check-3d-ledger: 5 ids, layouts=$(grep -c '"stage": "layout"' "$OUT")"