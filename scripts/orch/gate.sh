#!/usr/bin/env bash
# gate.sh <logdir> <rowsfile> — run gate rows `name|expect|cmd` (expect: 0 | nonzero) from the
# current directory; one log per row, <logdir>/summary.txt with PASS/FAIL, exit 1 if any row fails.
# A row whose command cannot run still records its real exit code: SKIP never counts as a pass.
# A missing rows file, or one with no row, is exit 2: svc-supply's job passed on a missing file
# (2026-10-04), because the loop read nothing and `fail` stayed 0.
set -uo pipefail
source "$(dirname "$(readlink -f "$0")")/docker-env.sh"
logdir=$1 rows=$2
mkdir -p "$logdir" || exit 2; summary=$logdir/summary.txt; : >"$summary"; fail=0 ran=0
while IFS='|' read -r name expect cmd; do
  [[ -z $name || $name == \#* ]] && continue
  start=$SECONDS ran=$((ran + 1))
  bash -c "$cmd" >"$logdir/$name.log" 2>&1; rc=$?
  case $expect in
    0) [[ $rc -eq 0 ]] && ok=PASS || ok=FAIL ;;
    nonzero) [[ $rc -ne 0 ]] && ok=PASS || ok=FAIL ;;
    *) echo "gate.sh: bad expect '$expect' in row $name" >&2; exit 2 ;;
  esac
  [[ $ok == FAIL ]] && fail=1
  printf '%s %-22s exit=%-3s expect=%-7s %ss\n' "$ok" "$name" "$rc" "$expect" $((SECONDS - start)) | tee -a "$summary"
done <"$rows"
((ran)) || { echo "gate.sh: no row ran from $rows" | tee -a "$summary" >&2; exit 2; }
exit "$fail"
