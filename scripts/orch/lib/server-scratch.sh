#!/usr/bin/env bash
# server-scratch.sh — the ONE copy of the workspace scratch setup that scripts/orch/lock-parity.sh
# and scripts/orch/svc-features.sh both use.
#
# WHY it exists: `scratch_setup` was a byte-identical function in both scripts after D7 made it loop
# over the workspace members. Two copies of the same forty lines is one copy too many: a fix to one
# is silently not a fix to the other, and the `--break` controls of the two scripts would then be
# testing different setups under the same name. This file is sourced, not executed.
#
# WHY the links are RELATIVE: `scripts/orch/gr` bind-mounts the repository at `/w`, so a link
# naming the host's own absolute path dangles inside the container and cargo reports "failed to read
# crates/graph-contract/Cargo.toml" — a failure that reads like a broken workspace rather than a bad
# link. `realpath --relative-to` computes each one against the directory the link lands in, which is
# also what makes the same function correct at two different scratch depths.
#
# WHY manifests are copied and sources linked, not `cp -r server`: the workspace's `target/` holds
# root-owned incremental locks a `cp -r` cannot read, so the copy fails before it reaches the lockfile
# or the feature. `cargo tree` reads manifests, so manifests copied is the whole of what has to change,
# and nothing under `server/` is edited.
#
# Exit: 0 the copy is in place · 2 it could not be made (or no members parsed)
#
# Caveat: the member list is parsed from a SINGLE-LINE `members = [...]`. A workspace that spells its
# members across several lines yields no members, and the function returns 2 rather than building a
# copy of nothing — which is the safe direction, but it means this file has to be revisited if
# `server/Cargo.toml` ever goes multi-line.
#
# Usage: from a script that has already `cd`-ed to the git top level and set `root`,
#   source scripts/orch/lib/server-scratch.sh
#   scratch_setup target/lock-parity/break-version || exit 2

# scratch_setup <scratch-dir> — build a scratch copy of `server/` that cargo can resolve.
#
# WHY the member list is read from the manifest and not written out: a second member
# (server/graph-store) joined `server/Cargo.toml`, and a copy that named one member was a workspace
# cargo cannot resolve — an error that reads like a broken workspace rather than a stale list.
scratch_setup() {
  local scratch=$1 link member members
  members=$(sed -n 's/^members = \[\(.*\)\]$/\1/p' server/Cargo.toml | tr -d '"' | tr ',' ' ')
  if [ -z "$members" ]; then
    echo "server-scratch: no members parsed from server/Cargo.toml" >&2
    return 2
  fi
  rm -rf "$scratch" || return 2
  mkdir -p "$scratch/server" || return 2
  for member in $members; do
    mkdir -p "$scratch/server/$member" || return 2
    cp "server/$member/Cargo.toml" "$scratch/server/$member/" || return 2
    link=$(realpath --relative-to="$scratch/server/$member" "$root/server/$member/src") || return 2
    ln -s "$link" "$scratch/server/$member/src" || return 2
  done
  link=$(realpath --relative-to="$scratch" "$root/crates") || return 2
  ln -s "$link" "$scratch/crates" || return 2
  cp server/Cargo.toml server/Cargo.lock "$scratch/server/" || return 2
}
