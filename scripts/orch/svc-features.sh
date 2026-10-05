#!/usr/bin/env bash
# svc-features.sh — row `svc-features` (`docs/contract/service-api.md` "Verdict", condition 1):
# `server/` links graph-wasm with neither `probe` nor `threads`, and sets no float or codegen
# flag the root workspace lacks.
#
#   scripts/orch/svc-features.sh [--break]
#
# Two questions, one run:
#
#   1. `scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features -i graph-wasm`
#      names every feature on graph-wasm's edge into the server. `probe` is refused natively
#      by a `compile_error!` (crates/graph-wasm/src/service.rs:21) because `ingest/phases.rs`
#      writes a process-wide mark table without a lock; `threads` is the browser build's
#      shared-memory model and is built by scripts/orch/wasm-threads.sh, not linked here.
#   2. The flag half: `.cargo/config*`, `RUSTFLAGS`, and every `[profile.*]` under `server/`,
#      compared with the root's. The root sets no `target-cpu` and no `-C target-feature`
#      (scripts/orch/wasm-threads.sh sets those, for one artifact, in its own image), so
#      anything of the sort under `server/` is a difference in the bytes the digest compares.
#
# `--break` is the negative control: it copies `server/` under `target/`, adds `probe` to the
# copy's graph-wasm edge, and re-runs. The copy is never the real tree — a control that edits
# the tree it checks would leave the tree broken whether it passed or failed.
#
# Exit: 0 neither feature is on and no flag differs · 1 a feature is on or a flag differs ·
#       2 cargo could not run
#
# Writes: target/svc-features/ (the --break copy only)

set -uo pipefail
here=$(dirname "$(readlink -f "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root" || exit 2

break=0
while [ $# -gt 0 ]; do
  case "$1" in
    --break) break=1; shift ;;
    # The manual is the header, printed from the file so it cannot drift from the comment
    # block a reader sees before running anything (scripts/scigraphs-conformance.sh does this).
    --help|-h)
      sed -n "2,/^$/p" "$0" | sed -e 's/^# \{0,1\}//' -e '/^$/d'
      exit 0
      ;;
    *)
      echo "svc-features: unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

. "$here/lib/server-scratch.sh" || exit 2

# WHY 2 and not 1 for a cargo failure: a check that could not look is not a check that found
# nothing. A `nonzero` gate row would be satisfied by a toolchain that is simply absent.
tree=$(mktemp) || exit 2
trap 'rm -f "$tree"' EXIT
manifest=server/Cargo.toml
if [ "$break" = 1 ]; then
  scratch=target/svc-features
  scratch_setup "$scratch" || exit 2
  manifest=$scratch/server/Cargo.toml
  # The one edge the control perturbs, named here rather than at the call site so this script
  # and the reason in the header cannot disagree about what `--break` means.
  broken_feature=probe
  if ! grep -q 'graph-wasm = { path = "../../crates/graph-wasm" }' "$scratch/server/graph-server/Cargo.toml"; then
    echo "svc-features: no graph-wasm edge in the scratch copy to add $broken_feature to" >&2
    exit 2
  fi
  sed -i "s|graph-wasm = { path = \"../../crates/graph-wasm\" }|graph-wasm = { path = \"../../crates/graph-wasm\", features = [\"$broken_feature\"] }|" \
    "$scratch/server/graph-server/Cargo.toml" || exit 2
fi

if ! "$here/gr" cargo tree --manifest-path "$manifest" -e features -i graph-wasm >"$tree" 2>&1; then
  echo "svc-features: cargo tree could not run:" >&2
  cat "$tree" >&2
  exit 2
fi

status=0
for feature in probe threads; do
  if grep -q "graph-wasm feature \"$feature\"" "$tree"; then
    echo "svc-features: graph-wasm is linked with $feature in $manifest" >&2
    grep "graph-wasm feature \"$feature\"" "$tree" >&2
    status=1
  fi
done

# The flag half. Each pattern is searched under `server/` and, for the root's own files, at
# the top; a hit under `server/` that has no counterpart at the root is a difference.
flags='target-cpu|target-feature|RUSTFLAGS|force-fp-math|unsafe-math-checks|cpu='
if server_flags=$(grep -rnE "$flags" server --include='*.toml' --include='config*' 2>/dev/null); then
  if ! grep -rqE "$flags" Cargo.toml .cargo/config.toml 2>/dev/null; then
    echo "svc-features: server/ sets a float or codegen flag the root workspace lacks:" >&2
    echo "$server_flags" >&2
    status=1
  fi
fi

if [ "$status" = 0 ]; then
  echo "svc-features: neither probe nor threads is on, and no flag differs from the root"
fi
exit "$status"