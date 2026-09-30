#!/usr/bin/env bash
# wt-new.sh <branch> [base] — a ready worktree for one job, at $GM_SCRATCH/wt/<branch>: the branch
# (checked out when it exists locally or on origin, else created from [base], default
# origin/develop), the .claude submodule (a worktree otherwise has it empty, so a job loads no house
# rules, skills or tools), the node_modules the cli_oracles tests need, and the OpenCode kit files.
# Prints the worktree path. Exit 0 ready, 2 misuse or the worktree exists, else the failing step's code.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
source "$here/scratch.sh"
branch=${1:?usage: wt-new.sh <branch> [base]} base=${2:-origin/develop}
wt=$GM_SCRATCH/wt/$branch
[[ ! -e $wt ]] || {
  echo "wt-new: $wt exists" >&2
  exit 2
}
git fetch -q origin
if git show-ref -q --verify "refs/heads/$branch"; then
  git worktree add -q "$wt" "$branch"
elif git show-ref -q --verify "refs/remotes/origin/$branch"; then
  git worktree add -q --track -b "$branch" "$wt" "origin/$branch"
else
  git worktree add -q -b "$branch" "$wt" "$base"
fi
cd "$wt"
git submodule update -q --init .claude
"$here/node-slim.sh" npm ci --ignore-scripts >/dev/null
scripts/orch/oc-kit.sh --check >/dev/null || scripts/orch/oc-kit.sh
echo "$wt"
