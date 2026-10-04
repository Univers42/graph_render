#!/usr/bin/env bash
# test-gate.sh — behaviour tests for scripts/orch/gate.sh. Plain bash; every row is a local
# `true`/`false`/`cat`, so no container starts. GATE_BREAK=1 runs the pre-fix loop (no `|| [[ -n
# $name ]]`) and must fail: the negative control. Exits 0 when every case passes, 1 otherwise.
set -uo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
GATE=$here/gate.sh
tmp=$(mktemp -d "${TMPDIR:-/tmp}/gate.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
if [[ ${GATE_BREAK:-0} == 1 ]]; then
  sed 's/ || \[\[ -n \$name \]\]//' "$GATE" >"$tmp/gate.sh"
  cp "$here/docker-env.sh" "$tmp/"
  GATE=$tmp/gate.sh
fi
pass=0 fail=0
check() { if eval "$2"; then pass=$((pass + 1)); echo "ok    $1"; else fail=$((fail + 1)); echo "FAIL  $1"; fi; }

printf 'a|0|true\nlast|nonzero|false' >"$tmp/no-newline.rows"
bash "$GATE" "$tmp/l1" "$tmp/no-newline.rows" >/dev/null 2>&1; rc=$?
check "last row without newline runs" "grep -q '^PASS last ' '$tmp/l1/summary.txt'"
check "no-newline file exits 0" "[[ $rc -eq 0 ]]"

printf 'eat|0|cat\nafter|0|true\n' >"$tmp/stdin.rows"
bash "$GATE" "$tmp/l2" "$tmp/stdin.rows" >/dev/null 2>&1
check "a row reading stdin leaves the next row" "grep -q '^PASS after ' '$tmp/l2/summary.txt'"

printf '# only a comment\n' >"$tmp/empty.rows"
bash "$GATE" "$tmp/l3" "$tmp/empty.rows" >/dev/null 2>&1; rc=$?
check "a file with no row is exit 2" "[[ $rc -eq 2 ]]"

printf 'bad|0|false\n' >"$tmp/red.rows"
bash "$GATE" "$tmp/l4" "$tmp/red.rows" >/dev/null 2>&1; rc=$?
check "a red row is exit 1" "[[ $rc -eq 1 ]]"

echo "$pass passed, $fail failed"
((fail == 0))
