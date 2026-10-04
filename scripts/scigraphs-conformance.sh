#!/usr/bin/env bash
# scripts/scigraphs-conformance.sh — every SciGraphs layout compared byte by byte with the
# motor, measured over one fixture set both arms read, and drawn side by side.
#
#   scripts/scigraphs-conformance.sh [--break]
#
# Six steps, each a separate command because each runs in a different image:
#
#   1. graph-cli emit-conformance-fixtures  → conformance.jsonl, motor/<NAME>.f64 + .f32, motor.jsonl
#   2. harness/scigraphs-conformance.py --reference  (ge-python-oracle)  → ref/ for 23 names
#   3. harness/scigraphs-conformance.py --graphviz   (ge-graphviz-oracle) → ref/ for 9 names
#   4. harness/scigraphs-conformance.py --metrics    (ge-python-oracle)  → metrics.json, shapes/
#   5. harness/scigraphs-conformance/render.py       (gm-chromium)       → png/, sheet.png
#   6. graph-cli scigraphs-conformance              → target/gates/scigraphs-conformance.json
#
# Exit:  0 every row agrees with its pinned baseline · 1 a row does not · 2 could not run
#
# `--break` is the negative control: it re-emits with
# GM_MUTATE_SCIGRAPHS_CONFORMANCE=SPRING_3D, which flips one bit of one `f64` coordinate in
# that one row's file (bit 29 — the last bit an `f32` keeps, so the change is visible in both
# the `f64` and the `f32` file) and re-runs everything. The run must then exit 1, and it exits 1
# **only** after reading the judge's own log and finding SPRING_3D named in it — so the control
# detects a judge that passes everything, which a plain `expect nonzero` row cannot.
#
# Writes: target/scigraphs-conformance/ (fixtures, motor/, ref/, shapes/, png/, metrics.json,
#         conformance-baseline-proposed.rs)
#         target/gates/scigraphs-conformance.json
#         docs/measurements/scigraphs-conformance.md is written by hand from this run's numbers
#
# Build first: scripts/orch/gr cargo build --release -p graph-cli
#
# Ponytail: the images are the repo's pinned ones and nothing else — numpy 2.3.3, scipy 1.16.2,
# networkx 3.6, igraph 0.11.9, Graphviz 16.1.0, and the SciGraphs/ submodule reached through the
# same `$PWD:/w` mount every other oracle in this repository uses. Agreement here is bytes, so a
# different build of any of them would move the answer and there is nothing in this script to
# notice; that is what the recorded library versions in `ref/<NAME>.json` are for.

set -euo pipefail
# WHY steps 1-5 exit 2 on failure: under `set -e` the script would exit with the failing
# step's own code, and a step that exits 1 would satisfy the `--break` row's `test $? -eq 1`
# without the judge ever running. Measured 2026-10-03: the gm-chromium render died silently
# under a loaded host and turned the negative control red with no message.
trap 'rc=$?; printf "scigraphs-conformance: could not run: line %s exited %s\n" "$LINENO" "$rc" >&2; exit 2' ERR

root=$(git rev-parse --show-toplevel)
cd "$root"
dir=target/scigraphs-conformance
# The two oracle images this script builds itself; gm-chromium comes from image.sh below.
images="ge-python-oracle ge-graphviz-oracle"
break=0
# The one row the negative control perturbs. Named here rather than at the call site so the
# gate row and this script agree on which row `--break` is about.
broken_row=SPRING_3D
while [ $# -gt 0 ]; do
  case "$1" in
    --break) break=1; shift ;;
    # The manual is the header: printed from the file, so it cannot drift from the comment
    # block a reader sees before running anything.
    --help|-h)
      sed -n "2,/^$/p" "$0" | sed -e 's/^# \{0,1\}//' -e '/^$/d'
      exit 0
      ;;
    *) printf 'scigraphs-conformance: unknown option %s\n' "$1" >&2; exit 2 ;;
  esac
done

python_image() {
  "$root/scripts/orch/drun" --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-python-oracle "$@"
}
graphviz_image() {
  "$root/scripts/orch/drun" --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle "$@"
}
chromium_image() {
  "$root/scripts/orch/drun" --rm --user 0:0 -v "$PWD:/w" -w /w gm-chromium "$@"
}
build() {
  docker build -q -t ge-python-oracle -f docker/python-oracle.Dockerfile \
    --build-context nx="$nx" --build-context ig="$ig" . >/dev/null
  docker build -q -t ge-graphviz-oracle -f docker/graphviz-oracle.Dockerfile \
    --build-context gv="$gv" . >/dev/null
}

# `image.sh` sources `scratch.sh`, which exports GM_SCRATCH. Both are meant to be sourced, not
# run: their output is empty, so running them would leave this script building from a path it
# never learned.
# shellcheck source=scripts/orch/image.sh
source scripts/orch/image.sh
refs=$GM_SCRATCH/refs
nx=$refs/networkx-3.6 ig=$refs/igraph-0.11.9 gv=$refs/graphviz-16.1.0

# WHY ensure_image: `build` makes only the two oracle images. On a host without gm-chromium
# (every CI runner) step 5 asked Docker Hub for it, which has no such image.
ensure_image gm-chromium || { printf 'scigraphs-conformance: could not build gm-chromium\n' >&2; exit 2; }
for image in $images; do
  if ! docker image inspect "$image" >/dev/null 2>&1; then
    printf 'scigraphs-conformance: building %s\n' "$image" >&2
    build || { printf 'scigraphs-conformance: could not build %s\n' "$image" >&2; exit 2; }
  fi
done

# 1. the motor arm: the fixtures both sides read, and every motor layout's own coordinates
if [ "$break" = 1 ]; then
  printf 'scigraphs-conformance: --break perturbs %s (one bit of one coordinate)\n' "$broken_row"
  # `-e`, not a shell prefix: `gr` runs the command inside the image, so the variable has to be
  # passed through the docker environment or the emit would never see it.
  scripts/orch/gr -e GM_MUTATE_SCIGRAPHS_CONFORMANCE=$broken_row \
    cargo run -q --release -p graph-cli -- emit-conformance-fixtures --out "$dir"
else
  scripts/orch/gr cargo run -q --release -p graph-cli -- emit-conformance-fixtures --out "$dir"
fi

# 2. the SciGraphs arm: `apply_graph_layout` itself, for the 23 names this image hosts
python_image python3 harness/scigraphs-conformance.py --reference "$dir"

# 3. the Graphviz arm: the 9 names SciGraphs reaches through `scigraphs_utils`, which is
#    absent from both oracle images, so they take the engine's own points
graphviz_image python3 harness/scigraphs-conformance.py --graphviz "$dir"

# 4. the metrics, the SVGs and the proposed baseline
python_image python3 harness/scigraphs-conformance.py --metrics "$dir"

# 5. the pictures. Missing: no row is rendered, so a broken image in the matrix would read as
#    "nothing differed".
chromium_image python3 harness/scigraphs-conformance/render.py "$dir/shapes" --out "$dir/png"

# 6. the verdict. The exit code is this script's: 0, 1 or 2.
status=0
# The log lives beside the fixture directory, not inside it: the fixture tree is written by
# containers running as root, so the host shell cannot create a file in there.
log=target/scigraphs-conformance-judge.log
scripts/orch/gr cargo run -q --release -p graph-cli -- scigraphs-conformance --dir "$dir" \
  >"$log" 2>&1 || status=$?
cat "$log"
if [ "$status" = 2 ]; then
  printf 'scigraphs-conformance: could not run (exit 2)\n' >&2
  exit 2
fi

# `--break` asserts **which** row failed, not merely that the judge returned nonzero.
#
# A bare `expect nonzero` row would be satisfied by the script turning a passing gate into exit
# 1 by hand, and a judge that passed everything unconditionally would then green *both* rows.
# So this path exits nonzero only when it has read the judge's own log and found the pinned row
# named in it — a judge that always passes leaves the log clean, this exits 0, and the negative
# control row turns red, which is the only way that row can detect it.
if [ "$break" = 1 ]; then
  if grep -q "^  $broken_row: FAIL .*motor bytes are not the pinned ones" "$log"; then
    printf 'scigraphs-conformance: --break caught: %s\n' "$broken_row"
    exit 1
  fi
  if [ "$status" = 0 ]; then
    printf 'scigraphs-conformance: --break did not fail the gate\n' >&2
    exit 0
  fi
  printf 'scigraphs-conformance: --break failed %d rows, and not on %s\n' \
    "$(grep -c ': FAIL ' "$log")" "$broken_row" >&2
  exit 2
fi
exit "$status"
