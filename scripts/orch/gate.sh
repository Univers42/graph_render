#!/usr/bin/env bash
# gate.sh <logdir> <rowsfile> — run gate rows `name|expect|cmd` (expect: 0 | nonzero) from the
# current directory; one log per row, <logdir>/summary.txt with PASS/FAIL, exit 1 if any row fails.
# A row whose command cannot run still records its real exit code: SKIP never counts as a pass.
# A missing rows file, or one with no row, is exit 2: svc-supply's job passed on a missing file
# (2026-10-04), because the loop read nothing and `fail` stayed 0.
# The last row runs even without a trailing newline: `read` returns 1 on it, which silently dropped
# the final negctl of p12-t3, p13-gv1, p13-gv1-circo and p13-gv1-patchwork (found by svc-r2-limits,
# 2026-10-04). A row's command reads /dev/null, never the rest of the rows file. test-gate.sh checks both.
#
# The image tag (docs/reviews/review-svc-r3.md new condition 3): a script that runs a container
# prints one line `image graph-motor:<16 hex>` into its log, and that tag is copied into the summary
# as ` image=<tag>`, so a PASS names the build it ran. A rows file carrying the line `#gate:image`
# asks for the field on every one of its rows: a row whose log holds no tag gets ` image=?` rather
# than no field at all, so a missing tag is visible in the summary. Consumers read $1 (PASS/FAIL)
# and $2 (the name); both, and the position of `exit=`, are unchanged by the field.
set -uo pipefail
source "$(dirname "$(readlink -f "$0")")/docker-env.sh"
logdir=$1 rows=$2
mkdir -p "$logdir" || exit 2; summary=$logdir/summary.txt; : >"$summary"; fail=0 ran=0
# `#gate:image` is the one directive a rows file may carry; every other `#` line is a comment.
image_wanted=0
grep -qx '#gate:image' "$rows" 2>/dev/null && image_wanted=1
while IFS='|' read -r name expect cmd || [[ -n $name ]]; do
  [[ -z $name || $name == \#* ]] && continue
  start=$SECONDS ran=$((ran + 1))
  bash -c "$cmd" >"$logdir/$name.log" 2>&1 </dev/null; rc=$?
  case $expect in
    0) [[ $rc -eq 0 ]] && ok=PASS || ok=FAIL ;;
    nonzero) [[ $rc -ne 0 ]] && ok=PASS || ok=FAIL ;;
    *) echo "gate.sh: bad expect '$expect' in row $name" >&2; exit 2 ;;
  esac
  [[ $ok == FAIL ]] && fail=1
  # The tag line, if the row's script printed one; the last such line wins, so a script may print
  # the tag of a leak negctl's rebuilt image after the real one.
  tag=$(sed -nE 's/^image (graph-motor:[0-9a-f]{16})$/\1/p' "$logdir/$name.log" | tail -n 1)
  image=
  [[ -n $tag ]] && image=" image=$tag"
  [[ -z $tag && $image_wanted == 1 ]] && image=' image=?'
  printf '%s %-22s exit=%-3s expect=%-7s %ss%s\n' "$ok" "$name" "$rc" "$expect" \
    $((SECONDS - start)) "$image" | tee -a "$summary"
done <"$rows"
((ran)) || { echo "gate.sh: no row ran from $rows" | tee -a "$summary" >&2; exit 2; }
exit "$fail"
