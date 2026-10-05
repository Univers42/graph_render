#!/usr/bin/env bash
# wt-new.sh <branch> [base] — a ready worktree for one job, at $GM_SCRATCH/wt/<branch>: the branch
# (checked out when it exists locally or on origin, else created from [base], default
# origin/develop), the SciGraphs submodule, the node_modules the cli_oracles tests need, and the
# kit's OpenCode wiring (`devil setup --only opencode`: untracked links into the installed plugin).
# Prints the worktree path. Exit 0 ready, 2 misuse or the worktree exists, else the failing step's code.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
source "$here/scratch.sh"
branch=${1:?usage: wt-new.sh <branch> [base]} base=${2:-origin/develop}
wt=$GM_SCRATCH/wt/$branch
# GM_WT_STORE (scratch.sh) puts the checkout on another disk and leaves $wt a symlink to it, so every
# script that names $GM_SCRATCH/wt/<branch> still finds it. Each target/ grows to 2-3 GB, which filled
# /home on 2026-10-02 while /mnt/storage had 476 GB free.
real=${GM_WT_STORE:+$GM_WT_STORE/$branch}
real=${real:-$wt}
[[ ! -e $wt && ! -L $wt && ! -e $real ]] || {
  echo "wt-new: $wt or $real exists" >&2
  exit 2
}
git fetch -q origin
if git show-ref -q --verify "refs/heads/$branch"; then
  git worktree add -q "$real" "$branch"
elif git show-ref -q --verify "refs/remotes/origin/$branch"; then
  git worktree add -q --track -b "$branch" "$real" "origin/$branch"
else
  git worktree add -q -b "$branch" "$real" "$base"
fi
[[ $real == "$wt" ]] || ln -s "$real" "$wt"
cd "$real"
# The host owns target/: under the rootful daemon a container creates it as root, and then the
# host-side logs of gate.sh and scigraphs-conformance.sh cannot be written (2026-10-02, fix-analysis).
mkdir -p target
# Borrow the main tree's SciGraphs objects, then copy them in (--dissociate), so the worktree
# never depends on that store: a fresh ssh clone ran past 15 min on 2026-10-05 (gate-full-cb11),
# the borrowed one took seconds.
ref=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")/SciGraphs
if git -C "$ref" rev-parse -q --git-dir >/dev/null 2>&1 && [[ -e $ref/.git ]]; then
  git submodule update -q --init --reference "$ref" --dissociate SciGraphs
else
  git submodule update -q --init SciGraphs
fi
"$here/node-slim.sh" npm ci --ignore-scripts >/dev/null
kit=${DEVIL_ROOT:-$HOME/.claude/plugins/marketplaces/univers42}
bash "$kit/tools/setup.sh" --apply --only opencode >/dev/null
echo "$wt"
