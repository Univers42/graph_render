#!/usr/bin/env bash
# test-gate.sh — behaviour tests for scripts/orch/gate.sh. Plain bash; every row is a local
# `true`/`false`/`echo`, so no container starts. GATE_BREAK=1 runs the pre-fix loop (no `|| [[ -n
# $name ]]`) and must fail: the negative control. Exits 0 when every case passes, 1 otherwise.
# The last four cases are the image tag of docs/reviews/review-svc-r3.md new condition 3: a printed
# tag is copied into the summary, `#gate:image` marks a missing one `image=?`, and a rows file
# without the directive gets no field at all.
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

# The image tag (docs/reviews/review-svc-r3.md new condition 3). A row's script prints one line
# `image graph-motor:<16 hex>`; gate.sh copies it into the summary, or writes `image=?` when the
# rows file asks for the field with `#gate:image` and the row printed no tag.
printf 'tagged|0|echo image graph-motor:0123456789abcdef\n' >"$tmp/tag.rows"
bash "$GATE" "$tmp/l5" "$tmp/tag.rows" >/dev/null 2>&1
check "a printed tag reaches the summary" \
  "grep -q '^PASS tagged .* image=graph-motor:0123456789abcdef\$' '$tmp/l5/summary.txt'"
check "a tag alone adds no expect field" \
  "grep -Eq '^PASS tagged +exit=0 +expect=0 +[0-9]+s image=graph-motor:0123456789abcdef\$' '$tmp/l5/summary.txt'"

printf '#gate:image\ntagged|0|echo image graph-motor:0123456789abcdef\nuntagged|0|echo no tag here\n' \
  >"$tmp/directive.rows"
bash "$GATE" "$tmp/l6" "$tmp/directive.rows" >/dev/null 2>&1
check "the directive keeps a printed tag" \
  "grep -q ' image=graph-motor:0123456789abcdef\$' '$tmp/l6/summary.txt'"
check "the directive marks a missing tag" \
  "grep -Eq '^PASS untagged +exit=0 +expect=0 +[0-9]+s image=\?\$' '$tmp/l6/summary.txt'"

printf 'untagged|0|true\n' >"$tmp/plain.rows"
bash "$GATE" "$tmp/l7" "$tmp/plain.rows" >/dev/null 2>&1
check "no directive, no field" "! grep -q 'image' '$tmp/l7/summary.txt'"

# The tag is read from the row's log, not from its name: a row whose command echoes a tag-looking
# line into a log the summary must not carry one unless it printed exactly one.
printf 'odd|0|echo " image graph-motor:0123456789abcdef"\n' >"$tmp/odd.rows"
bash "$GATE" "$tmp/l8" "$tmp/odd.rows" >/dev/null 2>&1
check "an indented tag line is not a tag" "! grep -q 'image=' '$tmp/l8/summary.txt'"

echo "$pass passed, $fail failed"
((fail == 0))
