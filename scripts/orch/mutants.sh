#!/usr/bin/env bash
# mutants.sh <base-rev> [extra cargo-mutants args] — cargo-mutants over the .rs diff base..HEAD of the
# current worktree, under the host-wide timed lock; prints the tally and every missed/timeout mutant.
set -uo pipefail
base=$1; shift
here=$(dirname "$(readlink -f "$0")")
top=$(git rev-parse --show-toplevel); cd "$top" || exit 1
mkdir -p target; git diff "$base"..HEAD -- '*.rs' >target/phase.diff
"$here/timed" env GR_IMAGE=ge-mutants CARGO_BUILD_JOBS=6 \
  "$here/gr" cargo mutants --in-diff target/phase.diff --jobs 3 -o target "$@" \
  >target/mutants-run.log 2>&1; rc=$?
out=target/mutants.out
echo "mutants rc=$rc"; for f in caught missed timeout unviable; do
  printf '%s=%s ' "$f" "$(wc -l <"$out/$f.txt" 2>/dev/null || echo '?')"; done; echo
cat "$out/missed.txt" "$out/timeout.txt" 2>/dev/null
exit "$rc"
