#!/usr/bin/env bash
# lock-parity.sh — row `lock-parity` (`docs/contract/service-api.md` "Verdict", condition 8):
# graph-wasm's whole normal-dependency closure resolves to one crate, one version and one
# feature set in both workspaces.
#
#   scripts/orch/lock-parity.sh [--break-version | --break-feature]
#
# The service is a separate workspace (`server/Cargo.toml`, whose header names this script) with
# its own lockfile. It reaches the motor by path, so a version or a feature can differ between
# the two without either manifest changing: two lockfiles, one motor, and no row that notices.
# This row is that row.
#
# Both sides are read the same way, from the same tool:
#
#   scripts/orch/gr cargo tree -e normal,features --prefix none -p graph-wasm --manifest-path <ws>
#
# `-e normal,features` gives the closure and the features on every edge; `--prefix none`
# strips the tree drawing, so the output is a sorted list of `name vX.Y.Z (path)` and
# `name feature "f"` lines; `-p graph-wasm` starts at the crate both workspaces share. The two
# lists are sorted and compared with `diff`, so a difference is printed rather than summarised.
#
# `--break-version` bumps one crate's version in a scratch copy's `server/Cargo.lock`, and
# `--break-feature` adds one feature to a scratch copy's graph-wasm edge. Both exit 1, and
# neither touches the real tree.
#
# Exit: 0 the two closures agree · 1 they differ · 2 cargo could not run
#
# Writes: target/lock-parity/ (the --break-* scratch copies only)

set -uo pipefail
here=$(dirname "$(readlink -f "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root" || exit 2

mode=plain
while [ $# -gt 0 ]; do
  case "$1" in
    --break-version|--break-feature) mode=${1#--}; shift ;;
    # The manual is the header, printed from the file so it cannot drift from the comment
    # block a reader sees before running anything (scripts/scigraphs-conformance.sh does this).
    --help|-h)
      sed -n "2,/^$/p" "$0" | sed -e 's/^# \{0,1\}//' -e '/^$/d'
      exit 0
      ;;
    *)
      echo "lock-parity: unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

# WHY the scratch copy copies manifests and LINKS `crates/` and `graph-server/src`, rather than
# `cp -r server`: `gr` mounts the repository at /w, so a link to the host's own path dangles
# inside the container; and `server/target/` holds root-owned incremental locks a `cp -r`
# cannot read, so the copy would fail before it reached the lockfile. `cargo tree` reads
# manifests, so manifests copied is the whole of what has to change. The links are RELATIVE,
# because of the mount.
# D7: the ONE copy of this function lives in scripts/orch/lib/server-scratch.sh, which
# scripts/orch/svc-features.sh also sources. See that file for the WHY.
. "$here/lib/server-scratch.sh" || exit 2

# The closure of graph-wasm under `$manifest_path`, sorted. Non-zero when cargo could not run.
#
# WHY the trailing ` (/w/crates/...)` is stripped: condition 8 compares crate, version and
# enabled features, and cargo prints the manifest path of every path dependency. In a scratch
# copy that path is the scratch's, not the real `crates/`, so leaving it in would make every
# line of a path crate differ and the printed diff would be noise around the one line that
# moved. The path is dropped from both sides alike, so what is compared is exactly the tuple
# the condition names.
closure() {
  local manifest_path=$1 out=$2
  # Two cargo tree reads, both with `--prefix none`, concatenated into one sorted list.
  #
  # The first is the closure: every crate graph-wasm reaches normally, with the features on
  # each of their edges.
  #
  # The second exists because the first cannot see graph-wasm's OWN features. `-p graph-wasm`
  # makes graph-wasm the root of the printed tree, and cargo does not label the root's edges —
  # so `graph-wasm feature "probe"` is absent from the closure output and present only in the
  # inverted view, which is what prints a crate's incoming feature edges. Without the second
  # read a feature added to the service's graph-wasm edge would be invisible here, and the
  # condition's "enabled features" would be only the features of the crates below graph-wasm.
  {
    "$here/gr" cargo tree --manifest-path "$manifest_path" -e normal,features --prefix none \
      -p graph-wasm 2>"$out.err"
    "$here/gr" cargo tree --manifest-path "$manifest_path" -e features --prefix none \
      -i graph-wasm 2>>"$out.err" | grep '^graph-wasm feature'
  # WHY ` (command-line)` is stripped along with the path: cargo labels a feature edge with
    # why that feature is on, and that reason differs between the two workspaces for a reason
    # that has nothing to do with parity — graph-wasm is a workspace member in the root and a
    # path dependency in `server/`, so the same `default` feature carries a different label on
    # each side. The condition compares which features are enabled, not why cargo says so.
  } | sed -e 's| (/[^)]*)$||' -e 's| (command-line)$||' | LC_ALL=C sort >"$out"
}

# WHY 2 and not 1 when cargo cannot run: a check that could not look is not a check that found
# nothing, and a `nonzero` gate row would be satisfied by an absent toolchain.
root_tree=$(mktemp) || exit 2
server_tree=$(mktemp) || exit 2
scratch_tree=$(mktemp) || exit 2
trap 'rm -f "$root_tree" "$server_tree" "$scratch_tree" "$root_tree.err" "$server_tree.err" "$scratch_tree.err"' EXIT

server_manifest=server/Cargo.toml
if [ "$mode" != plain ]; then
  scratch=target/lock-parity/$mode
  scratch_setup "$scratch" || exit 2
  server_manifest=$scratch/server/Cargo.toml
fi

if ! closure Cargo.toml "$root_tree" || [ ! -s "$root_tree" ]; then
  echo "lock-parity: cargo tree could not run in the root workspace:" >&2
  cat "$root_tree.err" >&2
  exit 2
fi
if ! closure "$server_manifest" "$server_tree" || [ ! -s "$server_tree" ]; then
  echo "lock-parity: cargo tree could not run in $server_manifest:" >&2
  cat "$server_tree.err" >&2
  exit 2
fi

# `diff -u`, left against right, so the printed difference names the workspace each side is.
compare() {
  local left=$1 right=$2 what=$3
  if diff -u "$left" "$right"; then
    return 0
  fi
  echo "lock-parity: $what differs (the root workspace on the left, $server_manifest on the right)" >&2
  return 1
}

case "$mode" in
plain)
  compare "$root_tree" "$server_tree" "graph-wasm's normal closure"
  ;;
break-version)
  # One version moved in the scratch copy's own lockfile, inside that crate's own
  # `[[package]]` block. `indexmap` is named because it is in graph-core's closed allow-list
  # (crates/graph-core/Cargo.toml) and so is in the closure, and because the requirement there
  # is the caret `indexmap = "2.14"`, which admits several patch releases — so the moved
  # version resolves instead of failing.
  #
  # WHY the patch moves DOWN and not up, and why the checksum line goes with it: a version that
  # was never published cannot be selected, and cargo's refusal to select it is exit 2 — this
  # script's "could not run", which would satisfy no gate row and prove nothing about parity.
  # A stale checksum is the same failure one step later. Both edits together give what this
  # control is for: a lock cargo accepts, whose closure names a version the root's does not.
  moved=indexmap
  python3 - "$scratch/server/Cargo.lock" "$moved" <<'PY' || exit 2
import re, sys

# WHY python and not sed: the lock's `[[package]]` blocks are multi-line, so the version to
# move is the one inside the named crate's own block. A sed that matched a neighbouring block
# would make the lock unreadable rather than divergent, and cargo's complaint would read as a
# toolchain failure — exit 2, which is not what this control is for.
#
# WHY the checksum line is dropped rather than recomputed: a checksum is the crate file's own
# content hash, so the moved version's real one is not derivable from this file, and the old
# one is a hard error. Cargo recomputes it from the crate it downloads, which is the value it
# would have written had the lock never been edited.
path, crate = sys.argv[1], sys.argv[2]
blocks = open(path).read().split("[[package]]")
for at, block in enumerate(blocks):
    if f'name = "{crate}"' not in block:
        continue
    found = re.search(r'version = "\d+\.\d+\.(\d+)"', block)
    if found is None:
        sys.exit(f"no version line in the {crate} block")
    if int(found.group(1)) == 0:
        sys.exit(f"{crate} is at patch 0: there is no version below it to move to")
    # The patch digits are the only group carrying no quote, so the line's quoting survives.
    patch = lambda m: f'{m[1]}{m[2]}{int(m[3]) - 1}{m[4]}'
    edited = re.sub(r'(version = ")(\d+\.\d+\.)(\d+)(")', patch, block, count=1)
    blocks[at] = re.sub(r'^checksum = .*\n', "", edited, flags=re.M)
    break
else:
    sys.exit(f"no {crate} block in {path}")
open(path, "w").write("[[package]]".join(blocks))
PY
  # WHY this closure's failure is reported and still 2: a lock cargo cannot resolve has told
  # us nothing about parity, and a silent `|| exit 2` would hide which half went wrong.
  if ! closure "$server_manifest" "$scratch_tree"; then
    echo "lock-parity: the moved lock did not resolve, so the control proved nothing:" >&2
    cat "$scratch_tree.err" >&2
    exit 2
  fi
  compare "$root_tree" "$scratch_tree" "graph-wasm's normal closure after moving $moved"
  ;;
break-feature)
  # One feature added to the service's own graph-wasm edge, which is the only dependency edge
  # `server/` owns. `probe` is the feature condition 1 forbids natively, and `cargo tree`
  # resolves it without compiling, so the closure moves exactly as a build would see it.
  added=probe
  edge='graph-wasm = { path = "../../crates/graph-wasm" }'
  if ! grep -qF "$edge" "$scratch/server/graph-server/Cargo.toml"; then
    echo "lock-parity: no plain graph-wasm edge in the scratch copy to add $added to" >&2
    exit 2
  fi
  sed -i "s|$edge|graph-wasm = { path = \"../../crates/graph-wasm\", features = [\"$added\"] }|" \
    "$scratch/server/graph-server/Cargo.toml" || exit 2
  closure "$server_manifest" "$scratch_tree" || exit 2
  compare "$root_tree" "$scratch_tree" "graph-wasm's normal closure after adding $added"
  ;;
esac