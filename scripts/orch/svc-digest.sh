#!/usr/bin/env bash
# svc-digest.sh — the native arm of the digest manifest (`docs/contract/service-api.md` "Verdict",
# condition 7): `server/graph-server/tests/digest.rs` over the committed manifest.
#
#   scripts/orch/svc-digest.sh [--break]
#
# The test itself is the check; this wrapper exists for two reasons the test cannot cover.
#
# It runs the release build, because a digest row is 94 motor runs and the debug build is
# several times slower for the same bytes. The wasm arm (`scripts/orch/svc-digest-wasm.sh`)
# builds the same way, so the two arms differ in no setting but the target.
#
# And it maps cargo's exit codes onto this repo's three: cargo answers 101 for a failed test
# and for a failed build alike, so a gate row ending `test $? -eq 1` could not tell a moved
# digest from a workspace that would not compile. Here a red test is 1 and anything else is 2,
# and `--break` (which sets `GM_SVC_DIGEST_BREAK=1`, flipping one bit of every snapshot before
# it is hashed) is what proves 1 is reachable.
#
# Exit: 0 every row matches the manifest · 1 a row does not · 2 could not build or run
#
# Writes: nothing in the tree. `emit_the_manifest` in the test is the only writer of the
#         manifest, and it is ignored and needs GM_SVC_DIGEST_EMIT=1.

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
      echo "svc-digest: unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

# WHY the run is `--release` and not `--locked`: this row is about digests, and `svc-floor`
# owns the build and the lockfile. Pinning the lock here would only add a second failure mode
# to a row whose red must mean "a digest moved".
#
# WHY the build is a separate step: cargo answers 101 for a failed test AND for a failed
# compile, so one `cargo test` cannot tell this row's red from a workspace that would not
# build. `--no-run` first makes the compile its own exit, and the row's three codes are then
# honest: 1 means a digest moved and nothing else can.
if [ "$break" = 1 ]; then
  break_env=(-e GM_SVC_DIGEST_BREAK=1)
else
  break_env=()
fi
"$here/gr" "${break_env[@]}" cargo test --manifest-path server/Cargo.toml \
  -p graph-server --release --test digest --no-run || {
  echo "svc-digest: the digest test did not build" >&2
  exit 2
}
"$here/gr" "${break_env[@]}" cargo test --manifest-path server/Cargo.toml \
  -p graph-server --release --test digest
case $? in
0) echo "svc-digest: every manifest row matches" ;;
101)
  echo "svc-digest: a manifest row does not match the committed digest" >&2
  exit 1
  ;;
*)
  echo "svc-digest: could not run the digest test" >&2
  exit 2
  ;;
esac